//! Research runs, the latest check of each competitor's website, and prices read from websites.

use super::audit;
use crate::error::AppResult;
use crate::research::catalog::normalize;
use crate::research::crawl::{site_status, PageRecord};
use rusqlite::{params, Connection};
use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::HashMap;

pub fn create_run(conn: &Connection, label: &str, lat: f64, lon: f64, radius_km: f64, types: &[String]) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO research_runs (started_at, status, center_label, center_lat, center_lon, radius_km, types) VALUES (?1, 'running', ?2, ?3, ?4, ?5, ?6)",
        params![super::now_utc(), label, lat, lon, radius_km, serde_json::to_string(types)?],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn finish_run(conn: &Connection, id: i64, status: &str, summary: Option<&serde_json::Value>, error: Option<&str>) -> AppResult<()> {
    conn.execute(
        "UPDATE research_runs SET finished_at = ?1, status = ?2, summary = ?3, error = ?4 WHERE id = ?5",
        params![super::now_utc(), status, summary.map(|s| s.to_string()), error, id],
    )?;
    audit(conn, "research_run", Some(id), status, &format!("Market research {status}"), None)?;
    Ok(())
}

#[derive(Serialize, Clone, Debug)]
pub struct RunRow {
    pub id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub center_label: String,
    pub radius_km: f64,
    pub types: Vec<String>,
    pub summary: Option<serde_json::Value>,
    pub error: Option<String>,
}

pub fn runs(conn: &Connection, limit: i64) -> AppResult<Vec<RunRow>> {
    let mut st = conn.prepare("SELECT id, started_at, finished_at, status, center_label, radius_km, types, summary, error FROM research_runs ORDER BY id DESC LIMIT ?1")?;
    let rows = st.query_map([limit], |r| {
        Ok(RunRow {
            id: r.get(0)?,
            started_at: r.get(1)?,
            finished_at: r.get(2)?,
            status: r.get(3)?,
            center_label: r.get(4)?,
            radius_km: r.get(5)?,
            types: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(),
            summary: r.get::<_, Option<String>>(7)?.and_then(|s| serde_json::from_str(&s).ok()),
            error: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Runs left "running" by a crash or a closed app are marked failed when the workspace opens.
pub fn close_stale_runs(conn: &Connection) -> AppResult<()> {
    conn.execute("UPDATE research_runs SET status = 'failed', finished_at = ?1, error = 'The app closed before the search finished.' WHERE status = 'running'", [super::now_utc()])?;
    Ok(())
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SiteCheck {
    pub status: String,
    pub checked_at: String,
    pub prices_found: i64,
    pub note: String,
}

/// Latest website check per competitor: overall status, when, prices found, and the most useful note.
pub fn site_checks(conn: &Connection) -> AppResult<HashMap<i64, SiteCheck>> {
    let mut st = conn.prepare("SELECT competitor_id, status, prices_found, checked_at, note FROM research_pages ORDER BY id")?;
    let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?)))?;
    let mut by: HashMap<i64, Vec<(String, i64, String, String)>> = HashMap::new();
    for row in rows {
        let (c, s, n, at, note) = row?;
        by.entry(c).or_default().push((s, n, at, note));
    }
    Ok(by
        .into_iter()
        .filter_map(|(c, pages)| {
            let status = site_status(pages.iter().map(|p| (p.0.as_str(), p.1 as usize)))?;
            let note = pages.iter().find(|p| p.0 == status && !p.3.is_empty()).or_else(|| pages.iter().find(|p| !p.3.is_empty())).map(|p| p.3.clone()).unwrap_or_default();
            Some((c, SiteCheck { status: status.to_string(), checked_at: pages.iter().map(|p| p.2.clone()).max().unwrap_or_default(), prices_found: pages.iter().map(|p| p.1).sum(), note }))
        })
        .collect())
}

#[derive(Serialize, Clone, Debug)]
pub struct PageRow {
    pub url: String,
    pub status: String,
    pub http_status: Option<i64>,
    pub prices_found: i64,
    pub consultation: Vec<String>,
    pub note: String,
    pub checked_at: String,
}

pub fn pages(conn: &Connection, competitor_id: i64) -> AppResult<Vec<PageRow>> {
    let mut st = conn.prepare("SELECT url, status, http_status, prices_found, consultation, note, checked_at FROM research_pages WHERE competitor_id = ?1 ORDER BY id")?;
    let rows = st.query_map([competitor_id], |r| {
        Ok(PageRow {
            url: r.get(0)?,
            status: r.get(1)?,
            http_status: r.get(2)?,
            prices_found: r.get(3)?,
            consultation: serde_json::from_str(&r.get::<_, String>(4)?).unwrap_or_default(),
            note: r.get(5)?,
            checked_at: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The newest website check time for a competitor, if any.
pub fn last_checked(conn: &Connection, competitor_id: i64) -> AppResult<Option<String>> {
    Ok(conn.query_row("SELECT MAX(checked_at) FROM research_pages WHERE competitor_id = ?1", [competitor_id], |r| r.get(0))?)
}

#[derive(Debug, Clone, PartialEq)]
pub struct WebPrice {
    pub service_id: Option<i64>,
    pub label: String,
    pub price: Decimal,
    pub price_type: String,
    pub price_max: Option<Decimal>,
    pub duration_min: Option<i64>,
    pub hair_length: String,
    pub stylist_level: String,
    pub source_url: String,
    pub observed_on: String,
    pub retrieved_at: String,
    pub category: String,
    pub canonical: String,
    pub description: String,
    pub raw_text: String,
}

/// Record a website check. `prices` is Some only when the site was actually read; then the
/// competitor's earlier website prices are replaced, keeping your exclusions and the services you
/// matched them to by hand. A failed check (site down, blocked) leaves earlier prices in place.
pub fn save_site(conn: &Connection, competitor_id: i64, run_id: Option<i64>, pages: &[PageRecord], prices: Option<&[WebPrice]>) -> AppResult<usize> {
    conn.execute("DELETE FROM research_pages WHERE competitor_id = ?1", [competitor_id])?;
    for p in pages {
        conn.execute(
            "INSERT INTO research_pages (competitor_id, run_id, url, status, http_status, prices_found, consultation, note, checked_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![competitor_id, run_id, p.url, p.status, p.http_status, p.prices_found as i64, serde_json::to_string(&p.consultation)?, p.note, p.checked_at],
        )?;
    }
    let Some(prices) = prices else { return Ok(0) };
    type Key = (String, String, String);
    let key = |label: &str, level: &str, length: &str| -> Key { (normalize(label), level.to_lowercase(), length.to_string()) };
    let mut kept: HashMap<Key, (bool, Option<Option<i64>>)> = HashMap::new();
    {
        let mut st = conn.prepare("SELECT service_label, stylist_level, hair_length, excluded, service_id, match_method FROM market_observations WHERE competitor_id = ?1 AND source_type = 'web_extracted'")?;
        let rows = st.query_map([competitor_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)? != 0, r.get::<_, Option<i64>>(4)?, r.get::<_, String>(5)?)))?;
        for row in rows {
            let (label, level, length, excluded, service, method) = row?;
            if excluded || method == "user" {
                kept.insert(key(&label, &level, &length), (excluded, (method == "user").then_some(service)));
            }
        }
    }
    conn.execute("DELETE FROM market_observations WHERE competitor_id = ?1 AND source_type = 'web_extracted'", [competitor_id])?;
    for p in prices {
        let (excluded, by_user) = kept.get(&key(&p.label, &p.stylist_level, &p.hair_length)).cloned().unwrap_or((false, None));
        let (service_id, method) = match by_user {
            Some(s) => (s, "user"),
            None => (p.service_id, if p.service_id.is_some() { "auto" } else { "none" }),
        };
        conn.execute(
            "INSERT INTO market_observations (competitor_id, service_id, service_label, price, price_type, price_max, duration_min, hair_length, stylist_level, source_type, source_url,
             observed_on, excluded, retrieved_at, category, canonical, description, raw_text, match_method)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'web_extracted',?10,?11,?12,?13,?14,?15,?16,?17,?18)",
            params![
                competitor_id,
                service_id,
                p.label,
                p.price.to_string(),
                p.price_type,
                p.price_max.map(|d| d.to_string()),
                p.duration_min,
                p.hair_length,
                p.stylist_level,
                p.source_url,
                p.observed_on,
                excluded as i64,
                p.retrieved_at,
                p.category,
                p.canonical,
                p.description,
                p.raw_text,
                method
            ],
        )?;
    }
    Ok(prices.len())
}

/// Match (or unmatch) an observation to one of your services by hand. Kept across re-checks.
pub fn set_match(conn: &Connection, observation_id: i64, service_id: Option<i64>) -> AppResult<()> {
    conn.execute("UPDATE market_observations SET service_id = ?1, match_method = 'user' WHERE id = ?2", params![service_id, observation_id])?;
    audit(conn, "market_observation", Some(observation_id), "match", "Observed price matched to a service", None)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::market::{competitors, observations, save_competitor, CompetitorInput};
    use crate::db::sales::tests::fixture;
    use rust_decimal_macros::dec;

    fn price(label: &str, p: Decimal, service: Option<i64>) -> WebPrice {
        WebPrice {
            service_id: service,
            label: label.into(),
            price: p,
            price_type: "exact".into(),
            price_max: None,
            duration_min: None,
            hair_length: String::new(),
            stylist_level: String::new(),
            source_url: "https://glow.example/services".into(),
            observed_on: "2026-10-11".into(),
            retrieved_at: "2026-10-11T18:00:00.000Z".into(),
            category: "color".into(),
            canonical: "root_touchup".into(),
            description: String::new(),
            raw_text: format!("{label} ${p}"),
        }
    }
    fn page(status: &'static str, n: usize) -> PageRecord {
        PageRecord { url: "https://glow.example/services".into(), status, http_status: Some(200), prices_found: n, consultation: vec!["Color correction".into()], note: String::new(), checked_at: "2026-10-11T18:00:00.000Z".into() }
    }

    #[test]
    fn rechecks_replace_prices_but_keep_your_choices() {
        let f = fixture();
        let c = CompetitorInput { id: None, name: "Glow".into(), address: String::new(), city: String::new(), state: String::new(), postal_code: String::new(), latitude: None, longitude: None, website: "https://glow.example".into(), phone: String::new(), notes: String::new() };
        let cid = save_competitor(&f.conn, &c, "manual", None, "").unwrap();
        save_site(&f.conn, cid, None, &[page("prices_found", 2)], Some(&[price("Root touch-up", dec!(70), Some(f.service)), price("Gloss", dec!(40), None)])).unwrap();
        let obs = observations(&f.conn, None).unwrap();
        assert_eq!(obs.len(), 2);
        let root = obs.iter().find(|o| o.input.service_label == "Root touch-up").unwrap();
        assert_eq!((root.source_type.as_str(), root.match_method.as_str(), root.retrieved_at.as_deref()), ("web_extracted", "auto", Some("2026-10-11T18:00:00.000Z")));
        // You exclude the gloss and unmatch the root touch-up.
        let gloss = obs.iter().find(|o| o.input.service_label == "Gloss").unwrap().input.id.unwrap();
        crate::db::market::set_excluded(&f.conn, gloss, true).unwrap();
        set_match(&f.conn, root.input.id.unwrap(), None).unwrap();
        // The next check finds new prices; your choices survive.
        save_site(&f.conn, cid, None, &[page("prices_found", 2)], Some(&[price("Root Touch-Up", dec!(75), Some(f.service)), price("Gloss", dec!(45), None)])).unwrap();
        let obs = observations(&f.conn, None).unwrap();
        let root = obs.iter().find(|o| o.input.service_label == "Root Touch-Up").unwrap();
        assert_eq!((root.input.price, root.input.service_id, root.match_method.as_str()), (dec!(75), None, "user"));
        assert!(obs.iter().find(|o| o.input.service_label == "Gloss").unwrap().excluded);
        // A failed check records the failure but keeps the prices.
        save_site(&f.conn, cid, None, &[page("unreachable", 0)], None).unwrap();
        assert_eq!(observations(&f.conn, None).unwrap().len(), 2);
        let checks = site_checks(&f.conn).unwrap();
        assert_eq!(checks[&cid].status, "unreachable");
        assert_eq!(competitors(&f.conn, false).unwrap()[0].research.as_ref().unwrap().status, "unreachable");
        assert_eq!(pages(&f.conn, cid).unwrap()[0].consultation, ["Color correction"]);
    }
}
