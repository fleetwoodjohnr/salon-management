//! Competitors, observed prices and market evidence for a service.

use super::core::{get_business, location};
use super::{audit, dec, opt_dec};
use crate::csvio::CsvTable;
use crate::domain::market::{evaluate, haversine_km, percentile_rank, recommend, DistanceUnit, Evidence, Obs, Recommendation, Rules, Stats};
use crate::domain::profile::Position;
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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
    /// Latest automatic check of the website (None: never checked).
    pub research: Option<super::research::SiteCheck>,
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
    let checks = super::research::site_checks(conn)?;
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
            research: None,
        })
    })?;
    let mut out: Vec<Competitor> = rows.collect::<Result<_, _>>()?;
    for c in &mut out {
        c.research = c.input.id.and_then(|id| checks.get(&id).cloned());
    }
    Ok(out)
}

/// A website as an absolute http(s) URL ("www.x.com" → "https://www.x.com/"); "" stays "".
/// Rejects other schemes, embedded credentials and absurd lengths.
pub fn normalize_website(raw: &str) -> AppResult<String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(String::new());
    }
    let with_scheme = if t.contains("://") { t.to_string() } else { format!("https://{}", t.trim_start_matches('/')) };
    let bad = || AppError::invalid("website", "Enter a web address like https://example.com.");
    let mut u = reqwest::Url::parse(&with_scheme).map_err(|_| bad())?;
    if !matches!(u.scheme(), "http" | "https") || u.host_str().is_none_or(|h| !h.contains('.')) || !u.username().is_empty() || u.password().is_some() || with_scheme.len() > 2048 {
        return Err(bad());
    }
    u.set_fragment(None);
    Ok(u.to_string())
}

pub fn save_competitor(conn: &Connection, c: &CompetitorInput, source: &str, source_ref: Option<&str>, attribution: &str) -> AppResult<i64> {
    if c.name.trim().is_empty() {
        return Err(AppError::invalid("name", "Enter the business name."));
    }
    let website = normalize_website(&c.website)?;
    let c = &CompetitorInput { website, ..c.clone() };
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

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Upserted {
    pub added: usize,
    /// Existing businesses whose empty fields were filled in.
    pub updated: usize,
    /// Found businesses recognised as one already listed (another source, or a duplicate map entry).
    pub merged: usize,
    /// Competitor id for each found business, in input order.
    pub ids: Vec<i64>,
}

/// Name for duplicate matching: lowercase letters and digits, "&" as "and", minus legal suffixes.
fn name_key(n: &str) -> String {
    n.to_lowercase()
        .replace('&', " and ")
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty() && !matches!(*w, "the" | "llc" | "inc" | "co" | "and"))
        .collect()
}

fn host_key(website: &str) -> Option<String> {
    reqwest::Url::parse(website).ok()?.host_str().map(|h| h.trim_start_matches("www.").to_lowercase())
}

fn phone_key(phone: &str) -> Option<String> {
    let d: String = phone.chars().filter(char::is_ascii_digit).collect();
    (d.len() >= 10).then(|| d[d.len() - 10..].to_string())
}

fn coords(c: &CompetitorInput) -> Option<(f64, f64)> {
    Some((c.latitude.as_deref()?.parse().ok()?, c.longitude.as_deref()?.parse().ok()?))
}

/// Add businesses found by a discovery source, merging duplicates instead of adding them twice.
/// A found business matches an existing one (archived included) by, in order: the same source
/// reference, the same website host, the same phone number, or the same name within 150 m (same
/// ZIP when either has no coordinates). Matches only get their *empty* fields filled in: what you
/// typed or corrected is never overwritten, and archived businesses stay archived.
pub fn upsert_discovered(conn: &Connection, found: &[(String, CompetitorInput)], source: &str, attribution: &str) -> AppResult<Upserted> {
    let mut known: Vec<(i64, Option<String>, CompetitorInput)> = {
        let mut st = conn.prepare("SELECT id, source_ref, name, address, city, state, postal_code, latitude, longitude, website, phone, notes FROM competitors")?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                CompetitorInput { id: r.get(0)?, name: r.get(2)?, address: r.get(3)?, city: r.get(4)?, state: r.get(5)?, postal_code: r.get(6)?, latitude: r.get(7)?, longitude: r.get(8)?, website: r.get(9)?, phone: r.get(10)?, notes: r.get(11)? },
            ))
        })?;
        rows.collect::<Result<_, _>>()?
    };
    let mut out = Upserted::default();
    for (src_ref, f) in found {
        let near = |k: &CompetitorInput| match (coords(k), coords(f)) {
            (Some((a, b)), Some((c, d))) => haversine_km(a, b, c, d) <= 0.15,
            _ => !f.postal_code.is_empty() && k.postal_code == f.postal_code,
        };
        let by_ref = known.iter().position(|(_, r, _)| r.as_deref() == Some(src_ref.as_str()));
        let hit = by_ref
            .or_else(|| host_key(&f.website).and_then(|h| known.iter().position(|(_, _, k)| host_key(&k.website).as_deref() == Some(h.as_str()))))
            .or_else(|| phone_key(&f.phone).and_then(|p| known.iter().position(|(_, _, k)| phone_key(&k.phone).as_deref() == Some(p.as_str()))))
            .or_else(|| known.iter().position(|(_, _, k)| name_key(&k.name) == name_key(&f.name) && near(k)));
        match hit {
            Some(i) => {
                let (id, r, k) = &mut known[i];
                let before = k.clone();
                for (mine, theirs) in [(&mut k.address, &f.address), (&mut k.city, &f.city), (&mut k.state, &f.state), (&mut k.postal_code, &f.postal_code), (&mut k.website, &f.website), (&mut k.phone, &f.phone)] {
                    if mine.trim().is_empty() && !theirs.is_empty() {
                        *mine = theirs.clone();
                    }
                }
                if k.latitude.is_none() || k.longitude.is_none() {
                    (k.latitude, k.longitude) = (f.latitude.clone(), f.longitude.clone());
                }
                let set_ref = r.is_none();
                if set_ref {
                    *r = Some(src_ref.clone());
                }
                if *k != before || set_ref {
                    conn.execute(
                        "UPDATE competitors SET address=?1, city=?2, state=?3, postal_code=?4, latitude=?5, longitude=?6, website=?7, phone=?8, source_ref=COALESCE(source_ref, ?9) WHERE id=?10",
                        params![k.address, k.city, k.state, k.postal_code, k.latitude, k.longitude, k.website, k.phone, src_ref, *id],
                    )?;
                }
                if by_ref.is_none() {
                    out.merged += 1;
                } else if *k != before {
                    out.updated += 1;
                }
                out.ids.push(*id);
            }
            None => {
                let id = save_competitor(conn, f, source, Some(src_ref), attribution)?;
                known.push((id, Some(src_ref.clone()), CompetitorInput { id: Some(id), ..f.clone() }));
                out.added += 1;
                out.ids.push(id);
            }
        }
    }
    Ok(out)
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
    /// "user_observed" | "csv_import" | "web_extracted"
    pub source_type: String,
    pub competitor_name: String,
    pub service_name: Option<String>,
    pub excluded: bool,
    pub distance_km: Option<f64>,
    /// How the match to your service was made: "user", "auto" (inferred) or "none".
    pub match_method: String,
    /// When the web page was read (website prices only).
    pub retrieved_at: Option<String>,
    pub category: String,
    pub canonical: String,
    pub description: String,
    /// The exact text the price was read from (website prices only).
    pub raw_text: String,
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
                 hair_length=?8, stylist_level=?9, inclusions=?10, source_url=?11, observed_on=?12, notes=?13,
                 match_method = CASE WHEN service_id IS ?2 THEN match_method WHEN ?2 IS NULL THEN 'none' ELSE 'user' END WHERE id=?14",
                p,
            )?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO market_observations (competitor_id, service_id, service_label, price, price_type, price_max, duration_min, hair_length, stylist_level,
                 inclusions, source_url, observed_on, notes, source_type, match_method, category, canonical)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
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
                    source_type,
                    if o.service_id.is_some() { "user" } else { "none" },
                    crate::research::catalog::classify(&o.service_label).map(|c| c.category).unwrap_or(""),
                    crate::research::catalog::classify(&o.service_label).map(|c| c.key).unwrap_or("")
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
                m.inclusions, m.source_url, m.observed_on, m.notes, m.source_type, c.name, s.name, m.excluded,
                m.match_method, m.retrieved_at, m.category, m.canonical, m.description, m.raw_text
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
            match_method: r.get(18)?,
            retrieved_at: r.get(19)?,
            category: r.get(20)?,
            canonical: r.get(21)?,
            description: r.get(22)?,
            raw_text: r.get(23)?,
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
    /// Only these businesses (None = all).
    #[serde(default)]
    pub competitor_ids: Option<Vec<i64>>,
    /// Only these sources: "user_observed", "csv_import", "web_extracted" (None = all).
    #[serde(default)]
    pub source_types: Option<Vec<String>>,
    #[serde(default)]
    pub price_min: Option<Decimal>,
    #[serde(default)]
    pub price_max: Option<Decimal>,
}

impl EvidenceQuery {
    pub fn for_service(service_id: i64) -> Self {
        EvidenceQuery {
            service_id,
            max_age_days: None,
            radius_km: None,
            hair_length: String::new(),
            stylist_level: String::new(),
            include_starting_at: false,
            position: None,
            target_price: None,
            competitor_ids: None,
            source_types: None,
            price_min: None,
            price_max: None,
        }
    }

    /// The price an observation is compared at: a range counts at its midpoint.
    fn compared(o: &ObservationInput) -> Decimal {
        match (o.price_type.as_str(), o.price_max) {
            ("range", Some(m)) => (o.price + m) / Decimal::TWO,
            _ => o.price,
        }
    }

    fn keeps(&self, o: &Observation) -> bool {
        let p = Self::compared(&o.input);
        self.competitor_ids.as_ref().is_none_or(|ids| ids.is_empty() || ids.contains(&o.input.competitor_id))
            && self.source_types.as_ref().is_none_or(|t| t.is_empty() || t.contains(&o.source_type))
            && self.price_min.is_none_or(|m| p >= m)
            && self.price_max.is_none_or(|m| p <= m)
    }
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
    pub your_price: Option<Decimal>,
    /// Where your price sits among the comparable prices (0–100).
    pub your_percentile: Option<i64>,
    /// Observed market price vs your cost-based target, combined.
    pub recommendation: Recommendation,
}

/// The market position that applies to a service: its own override, else its work profile's.
pub fn service_position(conn: &Connection, service_id: i64) -> AppResult<Position> {
    let v = super::services::get(conn, service_id)?;
    if let Some(p) = v.input.overrides.position {
        return Ok(p);
    }
    Ok(match v.input.profile_id {
        Some(pid) => super::profiles::get(conn, pid)?.data.position,
        None => Position::Standard,
    })
}

pub fn evidence(conn: &Connection, q: &EvidenceQuery, today: &str, cpi: Option<&HashMap<String, Decimal>>, unit: DistanceUnit) -> AppResult<MarketEvidence> {
    let obs: Vec<Observation> = observations(conn, Some(q.service_id))?.into_iter().filter(|o| q.keeps(o)).collect();
    let service = super::services::get(conn, q.service_id)?;
    let service_minutes = service.input.time.occupied_min();
    let rules = Rules {
        reference_date: today.into(),
        max_age_days: q.max_age_days.unwrap_or(540).clamp(30, 3650),
        radius_km: q.radius_km.filter(|r| *r > 0.0),
        hair_length: q.hair_length.clone(),
        stylist_level: q.stylist_level.clone(),
        include_starting_at: q.include_starting_at,
        target_minutes: Some(service_minutes),
        unit,
    };
    let input: Vec<Obs> = obs
        .iter()
        .map(|o| Obs {
            id: o.input.id.unwrap(),
            competitor_id: o.input.competitor_id,
            price: EvidenceQuery::compared(&o.input),
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
    let your_price = service.input.price;
    let included: Vec<Decimal> = {
        let mut v: Vec<Decimal> = ev.decisions.iter().filter(|d| d.included).filter_map(|d| input.iter().find(|o| o.id == d.id)).map(|o| o.price).collect();
        v.sort();
        v
    };
    let your_percentile = your_price.and_then(|p| percentile_rank(p, &included));
    let label = match q.position {
        Some(Position::Budget) => "lower-quartile",
        Some(Position::Premium) => "upper-quartile",
        Some(Position::Luxury) => "90th-percentile",
        _ => "median",
    };
    let recommendation = recommend(suggested, q.target_price, label);
    Ok(MarketEvidence { evidence: ev, rules, observations: obs, position: q.position, suggested, suggestion_note, conflict, cpi_adjusted_median, cpi_note, your_price, your_percentile, recommendation })
}

// ---------------------------------------------------------------- overview

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct OverviewQuery {
    pub max_age_days: Option<i64>,
    pub radius_km: Option<f64>,
    pub include_starting_at: bool,
    pub competitor_ids: Option<Vec<i64>>,
    pub source_types: Option<Vec<String>>,
    pub price_min: Option<Decimal>,
    pub price_max: Option<Decimal>,
    /// A service category key (research::catalog::CATEGORIES); None = all.
    pub category: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct OverviewRow {
    /// None for a service you don't offer (grouped by its normalised name).
    pub service_id: Option<i64>,
    pub canonical: Option<String>,
    pub name: String,
    pub category: String,
    pub your_price: Option<Decimal>,
    pub target_price: Option<Decimal>,
    pub position: Option<Position>,
    pub quality: String,
    pub stats: Option<Stats>,
    pub observations: usize,
    pub your_percentile: Option<i64>,
    pub recommendation: Option<Recommendation>,
    /// Comparable prices, for distribution charts.
    pub prices: Vec<Decimal>,
}

/// One row per service: your services (with your price, percentile and recommendation) and
/// services competitors offer that you don't, from automatically matched website prices.
pub fn overview(conn: &Connection, q: &OverviewQuery, services: &[super::services::ServiceSummary], today: &str, unit: DistanceUnit) -> AppResult<Vec<OverviewRow>> {
    let cat_of = |name: &str| crate::research::catalog::classify(name).map(|c| c.category).unwrap_or("other").to_string();
    let base = |id: i64| EvidenceQuery {
        max_age_days: q.max_age_days,
        radius_km: q.radius_km,
        include_starting_at: q.include_starting_at,
        competitor_ids: q.competitor_ids.clone(),
        source_types: q.source_types.clone(),
        price_min: q.price_min,
        price_max: q.price_max,
        ..EvidenceQuery::for_service(id)
    };
    let mut out = Vec::new();
    for s in services.iter().filter(|s| !s.archived) {
        let category = cat_of(&s.name);
        if q.category.as_ref().is_some_and(|c| *c != category) {
            continue;
        }
        let position = service_position(conn, s.id)?;
        let e = evidence(conn, &EvidenceQuery { position: Some(position), target_price: s.target_price, ..base(s.id) }, today, None, unit)?;
        let prices = e.evidence.decisions.iter().filter(|d| d.included).filter_map(|d| e.observations.iter().find(|o| o.input.id == Some(d.id))).map(|o| EvidenceQuery::compared(&o.input)).collect();
        out.push(OverviewRow {
            service_id: Some(s.id),
            canonical: crate::research::catalog::classify(&s.name).map(|c| c.key.to_string()),
            name: s.name.clone(),
            category,
            your_price: s.price,
            target_price: s.target_price,
            position: Some(position),
            quality: e.evidence.quality.clone(),
            stats: e.evidence.stats.clone(),
            observations: e.observations.len(),
            your_percentile: e.your_percentile,
            recommendation: Some(e.recommendation),
            prices,
        });
    }
    // Services you don't offer: unmatched observations grouped by normalised service.
    let comps: HashMap<i64, Option<f64>> = competitors(conn, true)?.into_iter().map(|c| (c.input.id.unwrap(), c.distance_km)).collect();
    let probe = base(0);
    let mut groups: std::collections::BTreeMap<String, Vec<Observation>> = std::collections::BTreeMap::new();
    for o in observations(conn, None)?.into_iter().filter(|o| o.input.service_id.is_none() && !o.canonical.is_empty() && probe.keeps(o)) {
        groups.entry(o.canonical.clone()).or_default().push(o);
    }
    let rules = Rules {
        reference_date: today.into(),
        max_age_days: q.max_age_days.unwrap_or(540).clamp(30, 3650),
        radius_km: q.radius_km.filter(|r| *r > 0.0),
        hair_length: String::new(),
        stylist_level: String::new(),
        include_starting_at: q.include_starting_at,
        target_minutes: None,
        unit,
    };
    for (key, obs) in groups {
        let Some(canon) = crate::research::catalog::by_key(&key) else { continue };
        if q.category.as_ref().is_some_and(|c| c != canon.category) || out.iter().any(|r: &OverviewRow| r.canonical.as_deref() == Some(key.as_str())) {
            continue;
        }
        let input: Vec<Obs> = obs
            .iter()
            .map(|o| Obs {
                id: o.input.id.unwrap(),
                competitor_id: o.input.competitor_id,
                price: EvidenceQuery::compared(&o.input),
                price_type: o.input.price_type.clone(),
                observed_on: o.input.observed_on.clone(),
                distance_km: comps.get(&o.input.competitor_id).copied().flatten(),
                hair_length: o.input.hair_length.clone(),
                stylist_level: o.input.stylist_level.clone(),
                duration_min: o.input.duration_min,
                excluded_by_user: o.excluded,
            })
            .collect();
        let ev = evaluate(&input, &rules);
        let prices = ev.decisions.iter().filter(|d| d.included).filter_map(|d| input.iter().find(|o| o.id == d.id)).map(|o| o.price).collect();
        out.push(OverviewRow {
            service_id: None,
            canonical: Some(key.clone()),
            name: canon.name.to_string(),
            category: canon.category.to_string(),
            your_price: None,
            target_price: None,
            position: None,
            quality: ev.quality.clone(),
            stats: ev.stats.clone(),
            observations: obs.len(),
            your_percentile: None,
            recommendation: None,
            prices,
        });
    }
    Ok(out)
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
        let q = EvidenceQuery { position: Some(Position::Standard), target_price: Some(dec!(95)), ..EvidenceQuery::for_service(f.service) };
        let e = evidence(&f.conn, &q, "2026-10-01", None, DistanceUnit::Km).unwrap();
        assert_eq!(e.evidence.quality, "medium");
        assert_eq!(e.suggested, Some(dec!(80)));
        assert!(e.conflict.as_ref().unwrap().contains("can't meet both"));
        // CPI modeled estimate is separate from the observed suggestion
        let cpi: HashMap<String, Decimal> = [("2026-09".to_string(), dec!(400)), ("2026-10".to_string(), dec!(404))].into();
        let e2 = evidence(&f.conn, &q, "2026-10-01", Some(&cpi), DistanceUnit::Km).unwrap();
        assert_eq!(e2.cpi_adjusted_median, Some(dec!(80.80)));
        assert_eq!(e2.suggested, Some(dec!(80)));
        // Recommendation: the market median is below the cost target, so costs win; filters narrow the set.
        assert_eq!((e.recommendation.recommended, e.recommendation.basis.as_str()), (Some(dec!(95)), "cost"));
        let few = evidence(&f.conn, &EvidenceQuery { price_max: Some(dec!(80)), ..q.clone() }, "2026-10-01", None, DistanceUnit::Km).unwrap();
        assert_eq!(few.observations.len(), 3);
        let one = evidence(&f.conn, &EvidenceQuery { competitor_ids: Some(vec![ids[0].0]), ..q.clone() }, "2026-10-01", None, DistanceUnit::Km).unwrap();
        assert_eq!(one.observations.len(), 1);
        assert_eq!(evidence(&f.conn, &EvidenceQuery { source_types: Some(vec!["web_extracted".into()]), ..q }, "2026-10-01", None, DistanceUnit::Km).unwrap().observations.len(), 0);
    }

    #[test]
    fn overview_has_your_services_and_ones_you_dont_offer() {
        let f = fixture();
        for (i, name) in ["A", "B", "C"].iter().enumerate() {
            let cid = save_competitor(&f.conn, &comp(name, 47.0, -122.9 + i as f64 * 0.01, "", ""), "manual", None, "").unwrap();
            let o = ObservationInput {
                id: None, competitor_id: cid, service_id: Some(f.service), service_label: "Root retouch".into(), price: Decimal::from(70 + 10 * i as i64), price_type: "exact".into(), price_max: None,
                duration_min: None, hair_length: String::new(), stylist_level: String::new(), inclusions: String::new(), source_url: String::new(), observed_on: "2026-09-20".into(), notes: String::new(),
            };
            save_observation(&f.conn, &o, "user_observed").unwrap();
            let web = crate::db::research::WebPrice {
                service_id: None, label: "Balayage".into(), price: Decimal::from(200 + 20 * i as i64), price_type: "exact".into(), price_max: None, duration_min: None, hair_length: String::new(),
                stylist_level: String::new(), source_url: "https://x.example/menu".into(), observed_on: "2026-10-01".into(), retrieved_at: "2026-10-01T10:00:00Z".into(), category: "specialty_color".into(),
                canonical: "balayage".into(), description: String::new(), raw_text: "Balayage $200".into(),
            };
            crate::db::research::save_site(&f.conn, cid, None, &[], Some(&[web])).unwrap();
        }
        let services = crate::db::services::list(&f.conn, false, crate::commands::services::tax_for).unwrap();
        let rows = overview(&f.conn, &OverviewQuery::default(), &services, "2026-10-11", DistanceUnit::Mi).unwrap();
        let mine = rows.iter().find(|r| r.service_id == Some(f.service)).unwrap();
        assert_eq!((mine.stats.as_ref().unwrap().n, mine.prices.len()), (3, 3));
        assert!(mine.recommendation.is_some());
        let bal = rows.iter().find(|r| r.canonical.as_deref() == Some("balayage")).unwrap();
        assert_eq!((bal.service_id, bal.stats.as_ref().unwrap().median, bal.category.as_str()), (None, dec!(220), "specialty_color"));
        // Filters: only website prices, only one category.
        let web_only = overview(&f.conn, &OverviewQuery { source_types: Some(vec!["web_extracted".into()]), ..Default::default() }, &services, "2026-10-11", DistanceUnit::Mi).unwrap();
        assert!(web_only.iter().find(|r| r.service_id == Some(f.service)).unwrap().stats.is_none());
        let nails = overview(&f.conn, &OverviewQuery { category: Some("nails".into()), ..Default::default() }, &services, "2026-10-11", DistanceUnit::Mi).unwrap();
        assert!(nails.iter().all(|r| r.category == "nails"));
    }

    fn comp(name: &str, lat: f64, lon: f64, website: &str, phone: &str) -> CompetitorInput {
        CompetitorInput { id: None, name: name.into(), address: String::new(), city: String::new(), state: String::new(), postal_code: String::new(), latitude: Some(lat.to_string()), longitude: Some(lon.to_string()), website: website.into(), phone: phone.into(), notes: String::new() }
    }

    #[test]
    fn discovered_businesses_merge_and_never_overwrite_edits() {
        let f = fixture();
        // Something the user added by hand, with their own spelling and website.
        let mine = save_competitor(&f.conn, &comp("The Glow Salon LLC", 47.0, -122.9, "glow.example", ""), "manual", None, "").unwrap();
        let found = vec![
            ("node/1".to_string(), comp("Glow Salon", 47.0005, -122.9, "", "(360) 555-0100")), // same name within 60 m → merge
            ("way/9".to_string(), comp("Glow Salon", 47.0006, -122.9001, "", "")), // the same place mapped twice → merge
            ("node/2".to_string(), comp("Shear Bliss", 47.01, -122.9, "https://www.shearbliss.example/", "")),
            ("node/3".to_string(), comp("Bliss Hair (new name)", 47.2, -122.9, "http://shearbliss.example/menu", "")), // same site host → merge
            ("node/4".to_string(), comp("Great Clips", 47.05, -122.9, "", "")),
            ("node/5".to_string(), comp("Great Clips", 47.09, -122.9, "", "")), // same chain 4 km away → separate
        ];
        let u = upsert_discovered(&f.conn, &found, "osm", "").unwrap();
        assert_eq!((u.added, u.merged), (3, 3), "{u:?}");
        assert_eq!(u.ids[0], mine);
        assert_eq!(u.ids[1], mine);
        let c = competitors(&f.conn, true).unwrap();
        let g = c.iter().find(|c| c.input.id == Some(mine)).unwrap();
        assert_eq!(g.input.name, "The Glow Salon LLC"); // user's name kept
        assert_eq!(g.input.website, "https://glow.example/"); // user's site kept (normalised on save)
        assert_eq!(g.input.phone, "(360) 555-0100"); // empty field filled
        assert_eq!(g.source, "manual");
        // Running the same search again adds nothing and changes nothing.
        let again = upsert_discovered(&f.conn, &found, "osm", "").unwrap();
        assert_eq!((again.added, again.updated), (0, 0), "{again:?}");
        assert_eq!(competitors(&f.conn, true).unwrap().len(), c.len());
        // Bad websites are refused, odd-but-valid ones normalised.
        assert!(normalize_website("javascript:alert(1)").is_err());
        assert!(normalize_website("https://user:pw@x.example").is_err());
        assert_eq!(normalize_website(" www.x.example/menu#top ").unwrap(), "https://www.x.example/menu");
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
