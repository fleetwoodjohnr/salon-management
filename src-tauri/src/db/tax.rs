//! Sales-tax configuration: versioned rate sets per location and versioned taxability decisions
//! per location and line category. See migrations/003_tax.sql for the model.

use super::core::{get_business, location};
use super::{audit, now_utc, opt_dec};
use crate::db::services::TaxContext;
use crate::domain::money::pct_below_100;
use crate::error::{AppError, AppResult};
use chrono::Datelike;
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RateSetInput {
    pub location_id: i64,
    pub state_rate: Option<Decimal>,
    pub county_rate: Option<Decimal>,
    pub city_rate: Option<Decimal>,
    pub district_rate: Option<Decimal>,
    /// Total %, required when components are not supplied; must equal their sum when they are.
    pub total_rate: Option<Decimal>,
    pub jurisdiction_label: String,
    pub jurisdiction_code: Option<String>,
    pub source: String,
    pub source_url: Option<String>,
    pub precision: String,
    pub status: String,
    pub effective_date: Option<String>,
    pub dataset_period: Option<String>,
    pub retrieved_at: Option<String>,
    pub note: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct RateSet {
    pub id: i64,
    pub location_id: i64,
    pub version: i64,
    pub state_rate: Option<Decimal>,
    pub county_rate: Option<Decimal>,
    pub city_rate: Option<Decimal>,
    pub district_rate: Option<Decimal>,
    pub total_rate: Decimal,
    pub jurisdiction_label: String,
    pub jurisdiction_code: Option<String>,
    pub source: String,
    pub source_url: Option<String>,
    pub precision: String,
    pub status: String,
    pub effective_date: Option<String>,
    pub dataset_period: Option<String>,
    pub retrieved_at: Option<String>,
    pub note: String,
    pub created_at: String,
    pub superseded_at: Option<String>,
    /// "current" | "stale" | "manual"
    pub freshness: String,
    pub freshness_note: String,
}

fn quarter_of(y: i32, m: u32) -> (i32, u32) {
    (y, (m - 1) / 3 + 1)
}

/// "Q42026" → (2026, 4)
fn parse_period(p: &str) -> Option<(i32, u32)> {
    let p = p.trim().to_uppercase();
    let q: u32 = p.strip_prefix('Q')?.get(0..1)?.parse().ok()?;
    let y: i32 = p.get(2..6)?.parse().ok()?;
    (1..=4).contains(&q).then_some((y, q))
}

pub fn freshness(source: &str, dataset_period: Option<&str>, retrieved_at: Option<&str>, created_at: &str, today: chrono::NaiveDate) -> (String, String) {
    let now_q = quarter_of(today.year(), today.month());
    if source == "manual" {
        return ("manual".into(), format!("Entered by you on {}. Check it whenever rates change (often at the start of a quarter).", &created_at[..10]));
    }
    if let Some((y, q)) = dataset_period.and_then(parse_period) {
        if (y, q) < now_q {
            return ("stale".into(), format!("The provider's data is for Q{q} {y}, but it is now Q{} {}. Refresh to get the current quarter's rate.", now_q.1, now_q.0));
        }
        return ("current".into(), format!("Provider data period Q{q} {y}."));
    }
    match retrieved_at.and_then(|r| chrono::DateTime::parse_from_rfc3339(r).ok()) {
        Some(t) => {
            let rq = quarter_of(t.year(), t.month());
            if rq < now_q {
                ("stale".into(), format!("Retrieved {} — a new quarter has started since. Refresh to confirm the rate.", &t.to_rfc3339()[..10]))
            } else {
                ("current".into(), format!("Retrieved {}. The provider didn't state which period its data covers.", &t.to_rfc3339()[..10]))
            }
        }
        None => ("stale".into(), "No retrieval date recorded.".into()),
    }
}

fn rate_from_row(r: &rusqlite::Row, today: chrono::NaiveDate) -> rusqlite::Result<RateSet> {
    let source: String = r.get("source")?;
    let dataset_period: Option<String> = r.get("dataset_period")?;
    let retrieved_at: Option<String> = r.get("retrieved_at")?;
    let created_at: String = r.get("created_at")?;
    let (freshness, freshness_note) = freshness(&source, dataset_period.as_deref(), retrieved_at.as_deref(), &created_at, today);
    Ok(RateSet {
        id: r.get("id")?,
        location_id: r.get("location_id")?,
        version: r.get("version")?,
        state_rate: opt_dec(r, "state_rate")?,
        county_rate: opt_dec(r, "county_rate")?,
        city_rate: opt_dec(r, "city_rate")?,
        district_rate: opt_dec(r, "district_rate")?,
        total_rate: super::dec(r, "total_rate")?,
        jurisdiction_label: r.get("jurisdiction_label")?,
        jurisdiction_code: r.get("jurisdiction_code")?,
        source,
        source_url: r.get("source_url")?,
        precision: r.get("precision")?,
        status: r.get("status")?,
        effective_date: r.get("effective_date")?,
        dataset_period,
        retrieved_at,
        note: r.get("note")?,
        created_at,
        superseded_at: r.get("superseded_at")?,
        freshness,
        freshness_note,
    })
}

pub fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

pub fn current_rate(conn: &Connection, location_id: i64) -> AppResult<Option<RateSet>> {
    let t = today();
    Ok(conn
        .query_row("SELECT * FROM tax_rate_sets WHERE location_id = ?1 AND superseded_at IS NULL ORDER BY version DESC LIMIT 1", [location_id], |r| rate_from_row(r, t))
        .optional()?)
}

pub fn rate_by_id(conn: &Connection, id: i64) -> AppResult<RateSet> {
    let t = today();
    conn.query_row("SELECT * FROM tax_rate_sets WHERE id = ?1", [id], |r| rate_from_row(r, t))
        .optional()?
        .ok_or_else(|| AppError::NotFound("Tax rate set not found.".into()))
}

pub fn rate_history(conn: &Connection, location_id: i64) -> AppResult<Vec<RateSet>> {
    let t = today();
    let mut st = conn.prepare("SELECT * FROM tax_rate_sets WHERE location_id = ?1 ORDER BY version DESC")?;
    let rows = st.query_map([location_id], |r| rate_from_row(r, t))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn save_rate(conn: &Connection, r: &RateSetInput) -> AppResult<i64> {
    location(conn, r.location_id)?;
    if !matches!(r.source.as_str(), "manual" | "wa_dor" | "ca_cdtfa") {
        return Err(AppError::invalid("source", "Unknown rate source."));
    }
    if !matches!(r.precision.as_str(), "address" | "zip9" | "zip5" | "city" | "county" | "state" | "unknown") {
        return Err(AppError::invalid("precision", "Say how precise this rate is."));
    }
    if !matches!(r.status.as_str(), "verified" | "estimate" | "manual") {
        return Err(AppError::invalid("status", "Unknown status."));
    }
    if r.source == "manual" && r.status == "verified" {
        return Err(AppError::invalid("status", "Manually entered rates are recorded as manual settings."));
    }
    let comps = [("state_rate", r.state_rate), ("county_rate", r.county_rate), ("city_rate", r.city_rate), ("district_rate", r.district_rate)];
    for (f, v) in comps {
        if let Some(v) = v {
            pct_below_100(f, "Rate", v)?;
        }
    }
    let any = comps.iter().any(|(_, v)| v.is_some());
    let sum: Decimal = comps.iter().filter_map(|(_, v)| *v).sum();
    let total = match (r.total_rate, any) {
        (Some(t), true) if t != sum => {
            return Err(AppError::invalid("total_rate", format!("The components add up to {}%, not {}%. Fix a component or leave the total blank.", sum.normalize(), t.normalize())))
        }
        (Some(t), _) => t,
        (None, true) => sum,
        (None, false) => return Err(AppError::invalid("total_rate", "Enter the total rate or its components.")),
    };
    pct_below_100("total_rate", "Total rate", total)?;
    for d in [&r.effective_date].into_iter().flatten() {
        crate::db::inventory::valid_date("effective_date", d)?;
    }
    let now = now_utc();
    conn.execute("UPDATE tax_rate_sets SET superseded_at = ?1 WHERE location_id = ?2 AND superseded_at IS NULL", params![now, r.location_id])?;
    let version: i64 = conn.query_row("SELECT COALESCE(MAX(version), 0) + 1 FROM tax_rate_sets WHERE location_id = ?1", [r.location_id], |x| x.get(0))?;
    let s = |d: Option<Decimal>| d.map(|d| d.normalize().to_string());
    conn.execute(
        "INSERT INTO tax_rate_sets (location_id, version, state_rate, county_rate, city_rate, district_rate, total_rate, jurisdiction_label,
         jurisdiction_code, source, source_url, precision, status, effective_date, dataset_period, retrieved_at, note, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
        params![
            r.location_id,
            version,
            s(r.state_rate),
            s(r.county_rate),
            s(r.city_rate),
            s(r.district_rate),
            total.normalize().to_string(),
            r.jurisdiction_label.trim(),
            r.jurisdiction_code,
            r.source,
            r.source_url,
            r.precision,
            r.status,
            r.effective_date,
            r.dataset_period,
            r.retrieved_at,
            r.note.trim(),
            now
        ],
    )?;
    let id = conn.last_insert_rowid();
    audit(conn, "tax_rate", Some(id), "create", &format!("Sales tax rate {}% set for location {} (source: {}, version {version})", total.normalize(), r.location_id, r.source), Some(serde_json::to_value(r)?))?;
    Ok(id)
}

/// Remove the current rate (e.g. it was wrong) without replacing it: sales become unresolved.
pub fn clear_rate(conn: &Connection, location_id: i64, note: &str) -> AppResult<()> {
    let n = conn.execute("UPDATE tax_rate_sets SET superseded_at = ?1 WHERE location_id = ?2 AND superseded_at IS NULL", params![now_utc(), location_id])?;
    if n == 0 {
        return Err(AppError::msg("There is no current rate to clear."));
    }
    audit(conn, "tax_rate", None, "clear", &format!("Sales tax rate cleared for location {location_id}: {note}"), None)?;
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TaxCategory {
    pub code: String,
    pub label: String,
    pub applies_to: String,
    pub builtin: bool,
}

pub fn categories(conn: &Connection) -> AppResult<Vec<TaxCategory>> {
    let mut st = conn.prepare("SELECT code, label, applies_to, builtin FROM tax_categories ORDER BY builtin DESC, label")?;
    let rows = st.query_map([], |r| Ok(TaxCategory { code: r.get(0)?, label: r.get(1)?, applies_to: r.get(2)?, builtin: r.get::<_, i64>(3)? != 0 }))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn add_category(conn: &Connection, label: &str, applies_to: &str) -> AppResult<String> {
    let label = label.trim();
    if label.is_empty() {
        return Err(AppError::invalid("label", "Name the category, e.g. Nail services."));
    }
    if !matches!(applies_to, "service" | "retail" | "other") {
        return Err(AppError::invalid("applies_to", "Choose services, retail or other."));
    }
    let code: String = label.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let exists: bool = conn.query_row("SELECT EXISTS (SELECT 1 FROM tax_categories WHERE code = ?1)", [&code], |r| r.get(0))?;
    if exists {
        return Err(AppError::invalid("label", "A category with this name already exists."));
    }
    conn.execute("INSERT INTO tax_categories (code, label, applies_to) VALUES (?1, ?2, ?3)", params![code, label, applies_to])?;
    audit(conn, "tax_category", None, "create", &format!("Tax category \"{label}\" added"), None)?;
    Ok(code)
}

#[derive(Serialize, Clone, Debug)]
pub struct TaxabilityRule {
    pub id: Option<i64>,
    pub category_code: String,
    pub category_label: String,
    pub applies_to: String,
    pub status: String,
    pub basis: String,
    pub decided_at: Option<String>,
}

pub fn current_rule(conn: &Connection, location_id: i64, category: &str) -> AppResult<Option<(i64, String, String, String)>> {
    Ok(conn
        .query_row(
            "SELECT id, status, basis, decided_at FROM taxability_rules WHERE location_id = ?1 AND category_code = ?2 AND superseded_at IS NULL ORDER BY id DESC LIMIT 1",
            params![location_id, category],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?)
}

pub fn rules(conn: &Connection, location_id: i64) -> AppResult<Vec<TaxabilityRule>> {
    categories(conn)?
        .into_iter()
        .map(|c| {
            let cur = current_rule(conn, location_id, &c.code)?;
            Ok(TaxabilityRule {
                id: cur.as_ref().map(|x| x.0),
                status: cur.as_ref().map(|x| x.1.clone()).unwrap_or("unknown".into()),
                basis: cur.as_ref().map(|x| x.2.clone()).unwrap_or_default(),
                decided_at: cur.map(|x| x.3),
                category_code: c.code,
                category_label: c.label,
                applies_to: c.applies_to,
            })
        })
        .collect()
}

pub fn set_rule(conn: &Connection, location_id: i64, category: &str, status: &str, basis: &str) -> AppResult<()> {
    location(conn, location_id)?;
    if !matches!(status, "taxable" | "exempt" | "unknown") {
        return Err(AppError::invalid("status", "Choose taxable, exempt or unknown."));
    }
    if status != "unknown" && basis.trim().is_empty() {
        return Err(AppError::invalid("basis", "Record where this decision comes from (e.g. the state revenue department page or your accountant)."));
    }
    let known: bool = conn.query_row("SELECT EXISTS (SELECT 1 FROM tax_categories WHERE code = ?1)", [category], |r| r.get(0))?;
    if !known {
        return Err(AppError::invalid("category_code", "Unknown tax category."));
    }
    conn.execute(
        "UPDATE taxability_rules SET superseded_at = ?1 WHERE location_id = ?2 AND category_code = ?3 AND superseded_at IS NULL",
        params![now_utc(), location_id, category],
    )?;
    conn.execute(
        "INSERT INTO taxability_rules (location_id, category_code, status, basis) VALUES (?1, ?2, ?3, ?4)",
        params![location_id, category, status, basis.trim()],
    )?;
    audit(conn, "taxability", Some(location_id), "set", &format!("{category} marked {status} at location {location_id}"), Some(serde_json::json!({ "basis": basis })))?;
    Ok(())
}

/// Pricing/checkout tax context for a line category at a location (default: primary location).
pub fn context_for(conn: &Connection, location_id: Option<i64>, category: &str) -> AppResult<TaxContext> {
    let biz = get_business(conn)?;
    let include = biz.prices_include_tax;
    let unresolved = |label: String| TaxContext { taxable: None, rate_pct: None, prices_include_tax: include, label, resolved: false };
    let Some(loc) = location_id.or(biz.primary_location_id) else {
        return Ok(unresolved("no business location is set".into()));
    };
    let cat_label: String = conn
        .query_row("SELECT label FROM tax_categories WHERE code = ?1", [category], |r| r.get(0))
        .optional()?
        .unwrap_or_else(|| category.to_string());
    match current_rule(conn, loc, category)?.map(|r| r.1).as_deref() {
        None | Some("unknown") => Ok(unresolved(format!("taxability of {} is not decided yet", cat_label.to_lowercase()))),
        Some("exempt") => Ok(TaxContext { taxable: Some(false), rate_pct: Some(Decimal::ZERO), prices_include_tax: include, label: format!("{cat_label}: not taxable"), resolved: true }),
        Some(_) => match current_rate(conn, loc)? {
            None => Ok(TaxContext { taxable: Some(true), rate_pct: None, prices_include_tax: include, label: "no sales tax rate is set for this location".into(), resolved: false }),
            Some(r) => Ok(TaxContext {
                taxable: Some(true),
                rate_pct: Some(r.total_rate),
                prices_include_tax: include,
                label: format!("{}% {}{}", r.total_rate.normalize(), r.jurisdiction_label, if r.freshness == "stale" { " (stale — refresh)" } else { "" }),
                resolved: true,
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::core::{save_business, save_location, Business, Location};
    use crate::db::open_memory;
    use rust_decimal_macros::dec;

    fn setup() -> (Connection, i64) {
        let conn = open_memory();
        let loc = save_location(
            &conn,
            &Location {
                id: None,
                name: "Main".into(),
                address_line: "6500 Linderson Way SW".into(),
                city: "Tumwater".into(),
                state: "WA".into(),
                postal_code: "98501".into(),
                latitude: None,
                longitude: None,
                geo_precision: None,
                geo_source: None,
                state_fips: None,
                county_fips: None,
                place_fips: None,
                archived: false,
            },
        )
        .unwrap();
        let mut b = Business::default();
        b.name = "S".into();
        b.primary_location_id = Some(loc);
        save_business(&conn, &b).unwrap();
        (conn, loc)
    }

    fn manual(loc: i64, total: Decimal) -> RateSetInput {
        RateSetInput {
            location_id: loc,
            state_rate: None,
            county_rate: None,
            city_rate: None,
            district_rate: None,
            total_rate: Some(total),
            jurisdiction_label: "Tumwater".into(),
            jurisdiction_code: None,
            source: "manual".into(),
            source_url: None,
            precision: "address".into(),
            status: "manual".into(),
            effective_date: None,
            dataset_period: None,
            retrieved_at: None,
            note: String::new(),
        }
    }

    #[test]
    fn unknown_never_becomes_zero() {
        let (conn, loc) = setup();
        let c = context_for(&conn, None, "service").unwrap();
        assert!(!c.resolved);
        assert_eq!(c.rate_pct, None);
        set_rule(&conn, loc, "service", "taxable", "WA DOR guidance").unwrap();
        let c = context_for(&conn, None, "service").unwrap();
        assert!(!c.resolved, "taxable but no rate is still unresolved");
        assert_eq!(c.rate_pct, None);
        save_rate(&conn, &manual(loc, dec!(9.8))).unwrap();
        let c = context_for(&conn, None, "service").unwrap();
        assert!(c.resolved);
        assert_eq!(c.rate_pct, Some(dec!(9.8)));
        set_rule(&conn, loc, "retail", "exempt", "test").unwrap();
        assert_eq!(context_for(&conn, None, "retail").unwrap().rate_pct, Some(dec!(0)));
        assert!(set_rule(&conn, loc, "retail", "taxable", "  ").is_err(), "a decision needs a basis");
    }

    #[test]
    fn rates_are_versioned_and_components_checked() {
        let (conn, loc) = setup();
        let mut r = manual(loc, dec!(9.8));
        r.state_rate = Some(dec!(6.5));
        r.city_rate = Some(dec!(3.2));
        assert!(save_rate(&conn, &r).is_err()); // 6.5 + 3.2 ≠ 9.8
        r.city_rate = Some(dec!(3.3));
        let first = save_rate(&conn, &r).unwrap();
        save_rate(&conn, &manual(loc, dec!(10.1))).unwrap();
        let h = rate_history(&conn, loc).unwrap();
        assert_eq!(h.len(), 2);
        assert_eq!(rate_by_id(&conn, first).unwrap().total_rate, dec!(9.8)); // old version intact
        assert_eq!(current_rate(&conn, loc).unwrap().unwrap().total_rate, dec!(10.1));
        clear_rate(&conn, loc, "wrong").unwrap();
        assert!(current_rate(&conn, loc).unwrap().is_none());
    }

    #[test]
    fn staleness_uses_the_dataset_period_not_the_fetch_time() {
        let today = chrono::NaiveDate::from_ymd_opt(2027, 1, 5).unwrap();
        let (f, _) = freshness("wa_dor", Some("Q42026"), Some("2027-01-05T10:00:00Z"), "2027-01-05T10:00:00Z", today);
        assert_eq!(f, "stale"); // fetched today, but the data is last quarter's
        let (f, _) = freshness("wa_dor", Some("Q12027"), Some("2027-01-05T10:00:00Z"), "2027-01-05T10:00:00Z", today);
        assert_eq!(f, "current");
        let (f, _) = freshness("ca_cdtfa", None, Some("2026-12-30T10:00:00Z"), "2026-12-30T10:00:00Z", today);
        assert_eq!(f, "stale");
        assert_eq!(freshness("manual", None, None, "2026-01-01T00:00:00Z", today).0, "manual");
    }
}
