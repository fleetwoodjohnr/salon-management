//! Competitors, observed prices and market evidence for a service.

use super::core::{get_business, location};
use super::{audit, dec, opt_dec};
use crate::csvio::CsvTable;
use crate::domain::market::{evaluate, haversine_km, Evidence, Obs, Rules};
use crate::domain::profile::Position;
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CompetitorInput {
    pub id: Option<i64>,
    pub name: String,
    pub address: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub latitude: Option<String>,
    pub longitude: Option<String>,
    pub website: String,
    pub phone: String,
    pub notes: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Competitor {
    pub input: CompetitorInput,
    pub source: String,
    pub source_ref: Option<String>,
    pub attribution: String,
    pub archived: bool,
    pub distance_km: Option<f64>,
    pub observations: i64,
}

fn origin(conn: &Connection) -> AppResult<Option<(f64, f64)>> {
    let Some(id) = get_business(conn)?.primary_location_id else { return Ok(None) };
    let l = location(conn, id)?;
    Ok(match (l.latitude.and_then(|v| v.parse().ok()), l.longitude.and_then(|v| v.parse().ok())) {
        (Some(a), Some(b)) => Some((a, b)),
        _ => None,
    })
}

pub fn competitors(conn: &Connection, include_archived: bool) -> AppResult<Vec<Competitor>> {
    let o = origin(conn)?;
    let mut st = conn.prepare(
        "SELECT c.id, c.name, c.address, c.city, c.state, c.postal_code, c.latitude, c.longitude, c.website, c.phone, c.notes, c.source, c.source_ref,
                c.attribution, c.archived_at, (SELECT COUNT(*) FROM market_observations m WHERE m.competitor_id = c.id)
         FROM competitors c WHERE (?1 OR c.archived_at IS NULL) ORDER BY c.name COLLATE NOCASE",
    )?;
    let rows = st.query_map([include_archived], |r| {
        let lat: Option<String> = r.get(6)?;
        let lon: Option<String> = r.get(7)?;
        let distance_km = match (o, lat.as_deref().and_then(|v| v.parse::<f64>().ok()), lon.as_deref().and_then(|v| v.parse::<f64>().ok())) {
            (Some((a, b)), Some(c), Some(d)) => Some(haversine_km(a, b, c, d)),
            _ => None,
        };
        Ok(Competitor {
            input: CompetitorInput {
                id: r.get(0)?,
                name: r.get(1)?,
                address: r.get(2)?,
                city: r.get(3)?,
                state: r.get(4)?,
                postal_code: r.get(5)?,
                latitude: lat,
                longitude: lon,
                website: r.get(8)?,
                phone: r.get(9)?,
                notes: r.get(10)?,
            },
            source: r.get(11)?,
            source_ref: r.get(12)?,
            attribution: r.get(13)?,
            archived: r.get::<_, Option<String>>(14)?.is_some(),
            distance_km,
            observations: r.get(15)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn save_competitor(conn: &Connection, c: &CompetitorInput, source: &str, source_ref: Option<&str>, attribution: &str) -> AppResult<i64> {
    if c.name.trim().is_empty() {
        return Err(AppError::invalid("name", "Enter the business name."));
    }
    for v in [&c.latitude, &c.longitude].into_iter().flatten() {
        v.parse::<f64>().map_err(|_| AppError::invalid("latitude", "Coordinates must be numbers."))?;
    }
    let id = match c.id {
        Some(id) => {
            conn.execute(
                "UPDATE competitors SET name=?1, address=?2, city=?3, state=?4, postal_code=?5, latitude=?6, longitude=?7, website=?8, phone=?9, notes=?10 WHERE id=?11",
                params![c.name.trim(), c.address, c.city, c.state, c.postal_code, c.latitude, c.longitude, c.website, c.phone, c.notes, id],
            )?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO competitors (name, address, city, state, postal_code, latitude, longitude, website, phone, notes, source, source_ref, attribution)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                params![c.name.trim(), c.address, c.city, c.state, c.postal_code, c.latitude, c.longitude, c.website, c.phone, c.notes, source, source_ref, attribution],
            )?;
            conn.last_insert_rowid()
        }
    };
    audit(conn, "competitor", Some(id), "save", &format!("Competitor \"{}\" saved", c.name.trim()), None)?;
    Ok(id)
}

/// Add or update businesses discovered in OpenStreetMap (keyed by OSM id). Returns (added, updated).
pub fn import_osm(conn: &Connection, found: &[(String, CompetitorInput)], attribution: &str) -> AppResult<(usize, usize)> {
    let (mut added, mut updated) = (0, 0);
    for (osm_ref, c) in found {
        let existing: Option<i64> = conn.query_row("SELECT id FROM competitors WHERE source='osm' AND source_ref=?1", [osm_ref], |r| r.get(0)).optional()?;
        match existing {
            Some(id) => {
                // Refresh location and contact data but keep the user's notes.
                conn.execute(
                    "UPDATE competitors SET name=?1, address=?2, city=?3, state=?4, postal_code=?5, latitude=?6, longitude=?7, website=?8, phone=?9 WHERE id=?10",
                    params![c.name, c.address, c.city, c.state, c.postal_code, c.latitude, c.longitude, c.website, c.phone, id],
                )?;
                updated += 1;
            }
            None => {
                save_competitor(conn, c, "osm", Some(osm_ref), attribution)?;
                added += 1;
            }
        }
    }
    Ok((added, updated))
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ObservationInput {
    pub id: Option<i64>,
    pub competitor_id: i64,
    pub service_id: Option<i64>,
    pub service_label: String,
    pub price: Decimal,
    pub price_type: String,
    pub price_max: Option<Decimal>,
    pub duration_min: Option<i64>,
    pub hair_length: String,
    pub stylist_level: String,
    pub inclusions: String,
    pub source_url: String,
    pub observed_on: String,
    pub notes: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Observation {
    pub input: ObservationInput,
    pub source_type: String,
    pub competitor_name: String,
    pub service_name: Option<String>,
    pub excluded: bool,
    pub distance_km: Option<f64>,
}

fn validate_obs(o: &ObservationInput) -> AppResult<()> {
    crate::db::inventory::valid_date("observed_on", &o.observed_on)?;
    if o.service_label.trim().is_empty() {
        return Err(AppError::invalid("service_label", "Write the service as the business lists it."));
    }
    if o.price <= Decimal::ZERO {
        return Err(AppError::invalid("price", "Enter the listed price."));
    }
    if !matches!(o.price_type.as_str(), "exact" | "starting_at" | "range") {
        return Err(AppError::invalid("price_type", "Choose exact, starting at, or range."));
    }
    if o.price_type == "range" && o.price_max.is_none_or(|m| m < o.price) {
        return Err(AppError::invalid("price_max", "A range needs a top price at least the bottom price."));
    }
    if !matches!(o.hair_length.as_str(), "" | "short" | "medium" | "long") {
        return Err(AppError::invalid("hair_length", "Unknown hair length."));
    }
    Ok(())
}

pub fn save_observation(conn: &Connection, o: &ObservationInput, source_type: &str) -> AppResult<i64> {
    validate_obs(o)?;
    let p = params![
        o.competitor_id,
        o.service_id,
        o.service_label.trim(),
        o.price.to_string(),
        o.price_type,
        o.price_max.map(|d| d.to_string()),
        o.duration_min,
        o.hair_length,
        o.stylist_level.trim(),
        o.inclusions.trim(),
        o.source_url.trim(),
        o.observed_on,
        o.notes,
        o.id
    ];
    let id = match o.id {
        Some(id) => {
            conn.execute(
                "UPDATE market_observations SET competitor_id=?1, service_id=?2, service_label=?3, price=?4, price_type=?5, price_max=?6, duration_min=?7,
                 hair_length=?8, stylist_level=?9, inclusions=?10, source_url=?11, observed_on=?12, notes=?13 WHERE id=?14",
                p,
            )?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO market_observations (competitor_id, service_id, service_label, price, price_type, price_max, duration_min, hair_length, stylist_level,
                 inclusions, source_url, observed_on, notes, source_type) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                params![
                    o.competitor_id,
                    o.service_id,
                    o.service_label.trim(),
                    o.price.to_string(),
                    o.price_type,
                    o.price_max.map(|d| d.to_string()),
                    o.duration_min,
                    o.hair_length,
                    o.stylist_level.trim(),
                    o.inclusions.trim(),
                    o.source_url.trim(),
                    o.observed_on,
                    o.notes,
                    source_type
                ],
            )?;
            conn.last_insert_rowid()
        }
    };
    audit(conn, "market_observation", Some(id), "save", &format!("Observed price ${} for \"{}\"", o.price, o.service_label.trim()), None)?;
    Ok(id)
}

pub fn set_excluded(conn: &Connection, id: i64, excluded: bool) -> AppResult<()> {
    conn.execute("UPDATE market_observations SET excluded = ?1 WHERE id = ?2", params![excluded as i64, id])?;
    Ok(())
}

pub fn delete_observation(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM market_observations WHERE id = ?1", [id])?;
    audit(conn, "market_observation", Some(id), "delete", "Observed price deleted", None)?;
    Ok(())
}

pub fn observations(conn: &Connection, service_id: Option<i64>) -> AppResult<Vec<Observation>> {
    let comps: HashMap<i64, Option<f64>> = competitors(conn, true)?.into_iter().map(|c| (c.input.id.unwrap(), c.distance_km)).collect();
    let mut st = conn.prepare(
        "SELECT m.id, m.competitor_id, m.service_id, m.service_label, m.price, m.price_type, m.price_max, m.duration_min, m.hair_length, m.stylist_level,
                m.inclusions, m.source_url, m.observed_on, m.notes, m.source_type, c.name, s.name, m.excluded
         FROM market_observations m JOIN competitors c ON c.id = m.competitor_id LEFT JOIN services s ON s.id = m.service_id
         WHERE (?1 IS NULL OR m.service_id = ?1) ORDER BY m.observed_on DESC, m.id DESC",
    )?;
    let rows = st.query_map([service_id], |r| {
        let cid: i64 = r.get(1)?;
        Ok(Observation {
            input: ObservationInput {
                id: r.get(0)?,
                competitor_id: cid,
                service_id: r.get(2)?,
                service_label: r.get(3)?,
                price: dec(r, "price")?,
                price_type: r.get(5)?,
                price_max: opt_dec(r, "price_max")?,
                duration_min: r.get(7)?,
                hair_length: r.get(8)?,
                stylist_level: r.get(9)?,
                inclusions: r.get(10)?,
                source_url: r.get(11)?,
                observed_on: r.get(12)?,
                notes: r.get(13)?,
            },
            source_type: r.get(14)?,
            competitor_name: r.get(15)?,
            service_name: r.get(16)?,
            excluded: r.get::<_, i64>(17)? != 0,
            distance_km: comps.get(&cid).copied().flatten(),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct EvidenceQuery {
    pub service_id: i64,
    pub max_age_days: Option<i64>,
    pub radius_km: Option<f64>,
    pub hair_length: String,
    pub stylist_level: String,
    pub include_starting_at: bool,
    pub position: Option<Position>,
    /// The cost-based target price, to check for a conflict with the market suggestion.
    pub target_price: Option<Decimal>,
}

#[derive(Serialize, Clone, Debug)]
pub struct MarketEvidence {
    pub evidence: Evidence,
    pub rules: Rules,
    pub observations: Vec<Observation>,
    pub position: Option<Position>,
    /// Observed-price suggestion for the position (only with medium or high evidence).
    pub suggested: Option<Decimal>,
    pub suggestion_note: String,
    pub conflict: Option<String>,
    /// Modeled (not observed): the median after adjusting each comparable to today's prices with
    /// the national CPI for haircuts and personal care, when CPI data has been downloaded.
    pub cpi_adjusted_median: Option<Decimal>,
    pub cpi_note: Option<String>,
}

pub fn evidence(conn: &Connection, q: &EvidenceQuery, today: &str, cpi: Option<&HashMap<String, Decimal>>) -> AppResult<MarketEvidence> {
    let obs = observations(conn, Some(q.service_id))?;
    let service_minutes = super::services::get(conn, q.service_id)?.input.time.occupied_min();
    let rules = Rules {
        reference_date: today.into(),
        max_age_days: q.max_age_days.unwrap_or(540).clamp(30, 3650),
        radius_km: q.radius_km.filter(|r| *r > 0.0),
        hair_length: q.hair_length.clone(),
        stylist_level: q.stylist_level.clone(),
        include_starting_at: q.include_starting_at,
        target_minutes: Some(service_minutes),
    };
    let input: Vec<Obs> = obs
        .iter()
        .map(|o| Obs {
            id: o.input.id.unwrap(),
            competitor_id: o.input.competitor_id,
            // A range counts at its midpoint.
            price: match (o.input.price_type.as_str(), o.input.price_max) {
                ("range", Some(m)) => (o.input.price + m) / Decimal::TWO,
                _ => o.input.price,
            },
            price_type: o.input.price_type.clone(),
            observed_on: o.input.observed_on.clone(),
            distance_km: o.distance_km,
            hair_length: o.input.hair_length.clone(),
            stylist_level: o.input.stylist_level.clone(),
            duration_min: o.input.duration_min,
            excluded_by_user: o.excluded,
        })
        .collect();
    let ev = evaluate(&input, &rules);
    let (suggested, suggestion_note) = match (&ev.stats, ev.quality.as_str(), q.position) {
        (Some(s), "medium" | "high", Some(p)) => {
            let (v, label) = match p {
                Position::Budget => (s.q1, "lower quartile"),
                Position::Standard => (s.median, "median"),
                Position::Premium => (s.q3, "upper quartile"),
                Position::Luxury => (s.p90, "90th percentile"),
            };
            (Some(crate::domain::money::round_money(v)), format!("The {label} of {} comparable observed prices.", s.n))
        }
        (Some(_), "low", _) => (None, "Too little evidence for a position-based suggestion; the observed range is shown for reference only.".into()),
        _ => (None, "Add prices you collect from local menus, or import them from a spreadsheet. Pricing continues from your costs meanwhile.".into()),
    };
    let conflict = match (suggested, q.target_price) {
        (Some(s), Some(t)) if s < t => Some(format!(
            "The market suggestion (${s:.2}) is below the price your costs and target need (${t:.2}). You can't meet both: charging ${s:.2} means a lower margin than your target."
        )),
        _ => None,
    };
    let (cpi_adjusted_median, cpi_note) = match (cpi, &ev.stats) {
        (Some(c), Some(_)) if !c.is_empty() => {
            let latest_key = c.keys().max().cloned().unwrap();
            let latest = c[&latest_key];
            let mut adj: Vec<Decimal> = Vec::new();
            for d in ev.decisions.iter().filter(|d| d.included) {
                let o = input.iter().find(|o| o.id == d.id).unwrap();
                if let Some(v) = c.get(&o.observed_on[..7]) {
                    adj.push(o.price * latest / v);
                }
            }
            adj.sort();
            if adj.is_empty() {
                (None, Some("CPI data doesn't cover these observation dates.".into()))
            } else {
                (
                    Some(crate::domain::money::round_money(crate::domain::market::quantile(&adj, Decimal::new(5, 1)))),
                    Some(format!("Modeled estimate: {} of the comparable prices adjusted to {} using the national CPI for haircuts and personal care services (BLS). Not an observed price.", adj.len(), latest_key)),
                )
            }
        }
        _ => (None, None),
    };
    Ok(MarketEvidence { evidence: ev, rules, observations: obs, position: q.position, suggested, suggestion_note, conflict, cpi_adjusted_median, cpi_note })
}

// ---------------------------------------------------------------- CSV import

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct ObservationMapping {
    pub business: Option<usize>,
    pub service_label: Option<usize>,
    pub price: Option<usize>,
    pub price_type: Option<usize>,
    pub price_max: Option<usize>,
    pub observed_on: Option<usize>,
    pub source_url: Option<usize>,
    pub hair_length: Option<usize>,
    pub stylist_level: Option<usize>,
    pub duration_min: Option<usize>,
    pub inclusions: Option<usize>,
    pub notes: Option<usize>,
    /// Column holding the name of your matching service (optional)
    pub matched_service: Option<usize>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ObsImportRow {
    pub line: usize,
    pub status: String,
    pub message: Option<String>,
    pub business: String,
    pub service_label: String,
    pub price: String,
    pub observed_on: String,
    pub matched_service: Option<String>,
}

fn cell(row: &[String], i: Option<usize>) -> String {
    i.and_then(|i| row.get(i)).cloned().unwrap_or_default().trim().to_string()
}

fn parse_obs_rows(conn: &Connection, t: &CsvTable, m: &ObservationMapping, default_service: Option<i64>) -> AppResult<Vec<(ObsImportRow, Option<(String, ObservationInput)>)>> {
    if m.business.is_none() || m.service_label.is_none() || m.price.is_none() || m.observed_on.is_none() {
        return Err(AppError::msg("Match at least the business, service, price and date columns."));
    }
    let services: HashMap<String, i64> = {
        let mut st = conn.prepare("SELECT lower(name), id FROM services WHERE archived_at IS NULL")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    let existing: std::collections::HashSet<(String, String, String, String)> = {
        let mut st = conn.prepare("SELECT lower(c.name), lower(m.service_label), m.price, m.observed_on FROM market_observations m JOIN competitors c ON c.id = m.competitor_id")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, String>(2)?, r.get(3)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (i, row) in t.rows.iter().enumerate() {
        let business = cell(row, m.business);
        let label = cell(row, m.service_label);
        let price_raw = cell(row, m.price).trim_start_matches('$').replace(',', "");
        let date = cell(row, m.observed_on);
        let ptype_raw = cell(row, m.price_type).to_lowercase();
        let matched = cell(row, m.matched_service);
        let service_id = if matched.is_empty() { default_service } else { services.get(&matched.to_lowercase()).copied() };
        let mut msg: Option<String> = None;
        let price: Option<Decimal> = price_raw.parse().ok();
        let ptype = match ptype_raw.as_str() {
            "" | "exact" | "fixed" => "exact",
            "from" | "starting at" | "starting_at" | "starts at" | "+" => "starting_at",
            "range" => "range",
            _ => {
                msg = Some(format!("Unknown price type \"{ptype_raw}\""));
                "exact"
            }
        };
        if business.is_empty() || label.is_empty() {
            msg = msg.or(Some("Business or service is empty".into()));
        }
        if price.is_none_or(|p| p <= Decimal::ZERO) {
            msg = msg.or(Some(format!("Price \"{price_raw}\" isn't a number")));
        }
        if chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").is_err() {
            msg = msg.or(Some(format!("Date \"{date}\" must look like 2026-09-30")));
        }
        if !matched.is_empty() && service_id.is_none() {
            msg = msg.or(Some(format!("No service named \"{matched}\"")));
        }
        let key = (business.to_lowercase(), label.to_lowercase(), price.map(|p| p.to_string()).unwrap_or_default(), date.clone());
        let status = if msg.is_some() {
            "error"
        } else if existing.contains(&key) || existing.contains(&(key.0.clone(), key.1.clone(), format!("{}", price.unwrap().normalize()), key.3.clone())) {
            msg = Some("Already recorded; will be skipped".into());
            "duplicate"
        } else if !seen.insert(key.clone()) {
            msg = Some("Repeated in this file; will be skipped".into());
            "duplicate"
        } else {
            "new"
        };
        let input = (status == "new").then(|| {
            (
                business.clone(),
                ObservationInput {
                    id: None,
                    competitor_id: 0,
                    service_id,
                    service_label: label.clone(),
                    price: price.unwrap(),
                    price_type: ptype.into(),
                    price_max: cell(row, m.price_max).trim_start_matches('$').parse().ok(),
                    duration_min: cell(row, m.duration_min).parse().ok(),
                    hair_length: cell(row, m.hair_length).to_lowercase(),
                    stylist_level: cell(row, m.stylist_level),
                    inclusions: cell(row, m.inclusions),
                    source_url: cell(row, m.source_url),
                    observed_on: date.clone(),
                    notes: cell(row, m.notes),
                },
            )
        });
        out.push((
            ObsImportRow {
                line: i + 2,
                status: status.into(),
                message: msg,
                business,
                service_label: label,
                price: price_raw,
                observed_on: date,
                matched_service: service_id.and_then(|id| services.iter().find(|(_, v)| **v == id).map(|(k, _)| k.clone())),
            },
            input,
        ));
    }
    Ok(out)
}

pub fn preview_observations(conn: &Connection, t: &CsvTable, m: &ObservationMapping, default_service: Option<i64>) -> AppResult<Vec<ObsImportRow>> {
    Ok(parse_obs_rows(conn, t, m, default_service)?.into_iter().map(|(r, _)| r).collect())
}

pub fn import_observations(conn: &Connection, t: &CsvTable, m: &ObservationMapping, default_service: Option<i64>) -> AppResult<usize> {
    let mut comps: HashMap<String, i64> = competitors(conn, true)?.into_iter().map(|c| (c.input.name.to_lowercase(), c.input.id.unwrap())).collect();
    let mut n = 0;
    for (row, input) in parse_obs_rows(conn, t, m, default_service)? {
        let Some((business, mut o)) = input else { continue };
        let cid = match comps.get(&business.to_lowercase()) {
            Some(id) => *id,
            None => {
                let id = save_competitor(
                    conn,
                    &CompetitorInput { id: None, name: business.clone(), address: String::new(), city: String::new(), state: String::new(), postal_code: String::new(), latitude: None, longitude: None, website: String::new(), phone: String::new(), notes: String::new() },
                    "csv",
                    None,
                    "",
                )?;
                comps.insert(business.to_lowercase(), id);
                id
            }
        };
        o.competitor_id = cid;
        save_observation(conn, &o, "csv_import").map_err(|e| AppError::msg(format!("Line {}: {e}", row.line)))?;
        n += 1;
    }
    audit(conn, "market_observation", None, "import", &format!("Imported {n} observed prices from CSV"), None)?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sales::tests::fixture;
    use rust_decimal_macros::dec;

    #[test]
    fn evidence_and_suggestion_with_conflict() {
        let f = fixture();
        let mut ids = vec![];
        for (i, name) in ["A", "B", "C", "D", "E"].iter().enumerate() {
            let c = CompetitorInput { id: None, name: name.to_string(), address: String::new(), city: String::new(), state: String::new(), postal_code: String::new(), latitude: None, longitude: None, website: String::new(), phone: String::new(), notes: String::new() };
            ids.push((save_competitor(&f.conn, &c, "manual", None, "").unwrap(), i));
        }
        for (cid, i) in &ids {
            save_observation(
                &f.conn,
                &ObservationInput {
                    id: None,
                    competitor_id: *cid,
                    service_id: Some(f.service),
                    service_label: "Root retouch".into(),
                    price: Decimal::from(70 + 5 * *i as i64),
                    price_type: "exact".into(),
                    price_max: None,
                    duration_min: None,
                    hair_length: String::new(),
                    stylist_level: String::new(),
                    inclusions: String::new(),
                    source_url: "https://example.com/menu".into(),
                    observed_on: "2026-09-01".into(),
                    notes: String::new(),
                },
                "user_observed",
            )
            .unwrap();
        }
        let q = EvidenceQuery { service_id: f.service, max_age_days: None, radius_km: None, hair_length: String::new(), stylist_level: String::new(), include_starting_at: false, position: Some(Position::Standard), target_price: Some(dec!(95)) };
        let e = evidence(&f.conn, &q, "2026-10-01", None).unwrap();
        assert_eq!(e.evidence.quality, "medium");
        assert_eq!(e.suggested, Some(dec!(80)));
        assert!(e.conflict.as_ref().unwrap().contains("can't meet both"));
        // CPI modeled estimate is separate from the observed suggestion
        let cpi: HashMap<String, Decimal> = [("2026-09".to_string(), dec!(400)), ("2026-10".to_string(), dec!(404))].into();
        let e2 = evidence(&f.conn, &q, "2026-10-01", Some(&cpi)).unwrap();
        assert_eq!(e2.cpi_adjusted_median, Some(dec!(80.80)));
        assert_eq!(e2.suggested, Some(dec!(80)));
    }

    #[test]
    fn csv_import_dedupes() {
        let f = fixture();
        let t = CsvTable {
            headers: vec!["Salon".into(), "Service".into(), "Price".into(), "Date".into(), "Type".into()],
            rows: vec![
                vec!["Glow".into(), "Root color".into(), "$85".into(), "2026-09-10".into(), "".into()],
                vec!["Glow".into(), "Root color".into(), "85".into(), "2026-09-10".into(), "".into()],
                vec!["Shine".into(), "Root color".into(), "from 70".into(), "2026-09-11".into(), "from".into()],
                vec!["Shine".into(), "Root color".into(), "70".into(), "09/11/2026".into(), "from".into()],
            ],
        };
        let m = ObservationMapping { business: Some(0), service_label: Some(1), price: Some(2), observed_on: Some(3), price_type: Some(4), ..Default::default() };
        let p = preview_observations(&f.conn, &t, &m, Some(f.service)).unwrap();
        let st: Vec<&str> = p.iter().map(|r| r.status.as_str()).collect();
        assert_eq!(st, vec!["new", "duplicate", "error", "error"]);
        assert_eq!(import_observations(&f.conn, &t, &m, Some(f.service)).unwrap(), 1);
        assert_eq!(preview_observations(&f.conn, &t, &m, Some(f.service)).unwrap()[0].status, "duplicate");
    }
}
