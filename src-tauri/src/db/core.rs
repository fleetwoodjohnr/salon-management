//! Business settings, locations, staff and the audit log.

use super::{audit, now_utc};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub fn get_setting<T: DeserializeOwned>(conn: &Connection, key: &str) -> AppResult<Option<T>> {
    let v: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
        .optional()?;
    Ok(v.map(|s| serde_json::from_str(&s)).transpose()?)
}

pub fn put_setting<T: Serialize>(conn: &Connection, key: &str, value: &T) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, serde_json::to_string(value)?],
    )?;
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DaySchedule {
    /// 0 = Monday … 6 = Sunday
    pub day: u8,
    pub open: bool,
    pub start: String,
    pub end: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Business {
    pub name: String,
    pub timezone: String,
    /// "us" (oz, fl oz, lb) or "metric" (g, ml, kg) — affects defaults and display, not storage.
    pub units: String,
    pub currency: String,
    pub schedule: Vec<DaySchedule>,
    pub primary_location_id: Option<i64>,
    pub prices_include_tax: bool,
    pub onboarding_completed: bool,
    pub onboarding_skipped: Vec<String>,
}

impl Default for Business {
    fn default() -> Self {
        Business {
            name: String::new(),
            timezone: "America/New_York".into(),
            units: "us".into(),
            currency: "USD".into(),
            schedule: (0..7)
                .map(|d| DaySchedule { day: d, open: d < 5, start: "09:00".into(), end: "17:00".into() })
                .collect(),
            primary_location_id: None,
            prices_include_tax: false,
            onboarding_completed: false,
            onboarding_skipped: vec![],
        }
    }
}

fn valid_hhmm(s: &str) -> bool {
    chrono::NaiveTime::parse_from_str(s, "%H:%M").is_ok()
}

pub fn get_business(conn: &Connection) -> AppResult<Business> {
    Ok(get_setting(conn, "business")?.unwrap_or_default())
}

pub fn save_business(conn: &Connection, b: &Business) -> AppResult<()> {
    if b.name.trim().is_empty() {
        return Err(AppError::invalid("name", "Enter a business name."));
    }
    if !matches!(b.units.as_str(), "us" | "metric") {
        return Err(AppError::invalid("units", "Units must be US or metric."));
    }
    if b.currency != "USD" {
        return Err(AppError::invalid("currency", "Only USD is supported in this version."));
    }
    for d in &b.schedule {
        if d.day > 6 || !valid_hhmm(&d.start) || !valid_hhmm(&d.end) {
            return Err(AppError::invalid("schedule", "Use 24-hour times like 09:00."));
        }
        if d.open && d.end <= d.start {
            return Err(AppError::invalid("schedule", "Closing time must be after opening time."));
        }
    }
    if let Some(id) = b.primary_location_id {
        location(conn, id)?;
    }
    put_setting(conn, "business", b)?;
    audit(conn, "business", None, "update", &format!("Business settings saved ({})", b.name), None)?;
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Location {
    pub id: Option<i64>,
    pub name: String,
    pub address_line: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub latitude: Option<String>,
    pub longitude: Option<String>,
    pub geo_precision: Option<String>,
    pub geo_source: Option<String>,
    pub state_fips: Option<String>,
    pub county_fips: Option<String>,
    pub place_fips: Option<String>,
    #[serde(default)]
    pub archived: bool,
}

const LOCATION_COLS: &str = "id, name, address_line, city, state, postal_code, latitude, longitude, geo_precision, geo_source, state_fips, county_fips, place_fips, archived_at";

fn location_row(r: &rusqlite::Row) -> rusqlite::Result<Location> {
    Ok(Location {
        id: r.get("id")?,
        name: r.get("name")?,
        address_line: r.get("address_line")?,
        city: r.get("city")?,
        state: r.get("state")?,
        postal_code: r.get("postal_code")?,
        latitude: r.get("latitude")?,
        longitude: r.get("longitude")?,
        geo_precision: r.get("geo_precision")?,
        geo_source: r.get("geo_source")?,
        state_fips: r.get("state_fips")?,
        county_fips: r.get("county_fips")?,
        place_fips: r.get("place_fips")?,
        archived: r.get::<_, Option<String>>("archived_at")?.is_some(),
    })
}

pub fn locations(conn: &Connection) -> AppResult<Vec<Location>> {
    let mut st = conn.prepare(&format!("SELECT {LOCATION_COLS} FROM locations ORDER BY archived_at IS NOT NULL, name"))?;
    let rows = st.query_map([], location_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn location(conn: &Connection, id: i64) -> AppResult<Location> {
    conn.query_row(&format!("SELECT {LOCATION_COLS} FROM locations WHERE id = ?1"), [id], location_row)
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("Location {id} not found.")))
}

pub fn save_location(conn: &Connection, l: &Location) -> AppResult<i64> {
    if l.name.trim().is_empty() {
        return Err(AppError::invalid("name", "Give the location a name."));
    }
    let state = l.state.trim().to_uppercase();
    if !state.is_empty() && (state.len() != 2 || !state.chars().all(|c| c.is_ascii_alphabetic())) {
        return Err(AppError::invalid("state", "Use the two-letter state code, e.g. WA."));
    }
    let zip = l.postal_code.trim();
    let zip_ok = zip.is_empty()
        || (zip.len() == 5 && zip.chars().all(|c| c.is_ascii_digit()))
        || (zip.len() == 10 && zip.as_bytes()[5] == b'-' && zip.chars().filter(|c| c.is_ascii_digit()).count() == 9);
    if !zip_ok {
        return Err(AppError::invalid("postal_code", "Use a 5-digit ZIP or ZIP+4 (12345-6789)."));
    }
    let now = now_utc();
    let p = params![
        l.name.trim(),
        l.address_line.trim(),
        l.city.trim(),
        state,
        zip,
        l.latitude,
        l.longitude,
        l.geo_precision,
        l.geo_source,
        l.state_fips,
        l.county_fips,
        l.place_fips,
        now,
        l.id
    ];
    let id = match l.id {
        Some(id) => {
            let n = conn.execute(
                "UPDATE locations SET name=?1, address_line=?2, city=?3, state=?4, postal_code=?5, latitude=?6, longitude=?7,
                 geo_precision=?8, geo_source=?9, state_fips=?10, county_fips=?11, place_fips=?12, updated_at=?13 WHERE id=?14",
                p,
            )?;
            if n == 0 {
                return Err(AppError::NotFound(format!("Location {id} not found.")));
            }
            audit(conn, "location", Some(id), "update", &format!("Location \"{}\" updated", l.name.trim()), None)?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO locations (name, address_line, city, state, postal_code, latitude, longitude, geo_precision, geo_source, state_fips, county_fips, place_fips, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                &p[..13],
            )?;
            let id = conn.last_insert_rowid();
            audit(conn, "location", Some(id), "create", &format!("Location \"{}\" created", l.name.trim()), None)?;
            id
        }
    };
    Ok(id)
}

/// Archive (hide from pickers, keep history) or restore a row in a table with `archived_at`.
pub fn set_archived(conn: &Connection, table: &str, entity: &str, id: i64, archived: bool) -> AppResult<()> {
    // `table` is never user input: callers pass a literal.
    let n = conn.execute(
        &format!("UPDATE {table} SET archived_at = ?1 WHERE id = ?2"),
        params![archived.then(now_utc), id],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("{entity} {id} not found.")));
    }
    audit(conn, entity, Some(id), if archived { "archive" } else { "restore" }, &format!("{entity} {id} {}", if archived { "archived" } else { "restored" }), None)?;
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Staff {
    pub id: Option<i64>,
    pub name: String,
    pub color: String,
    pub default_profile_id: Option<i64>,
    pub location_id: Option<i64>,
    #[serde(default)]
    pub archived: bool,
}

pub fn staff_list(conn: &Connection) -> AppResult<Vec<Staff>> {
    let mut st = conn.prepare(
        "SELECT id, name, color, default_profile_id, location_id, archived_at FROM staff ORDER BY archived_at IS NOT NULL, name",
    )?;
    let rows = st.query_map([], |r| {
        Ok(Staff {
            id: r.get(0)?,
            name: r.get(1)?,
            color: r.get(2)?,
            default_profile_id: r.get(3)?,
            location_id: r.get(4)?,
            archived: r.get::<_, Option<String>>(5)?.is_some(),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn save_staff(conn: &Connection, s: &Staff) -> AppResult<i64> {
    if s.name.trim().is_empty() {
        return Err(AppError::invalid("name", "Enter a name."));
    }
    match s.id {
        Some(id) => {
            conn.execute(
                "UPDATE staff SET name=?1, color=?2, default_profile_id=?3, location_id=?4 WHERE id=?5",
                params![s.name.trim(), s.color, s.default_profile_id, s.location_id, id],
            )?;
            audit(conn, "staff", Some(id), "update", &format!("Staff \"{}\" updated", s.name.trim()), None)?;
            Ok(id)
        }
        None => {
            conn.execute(
                "INSERT INTO staff (name, color, default_profile_id, location_id) VALUES (?1,?2,?3,?4)",
                params![s.name.trim(), s.color, s.default_profile_id, s.location_id],
            )?;
            let id = conn.last_insert_rowid();
            audit(conn, "staff", Some(id), "create", &format!("Staff \"{}\" added", s.name.trim()), None)?;
            Ok(id)
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct AuditEntry {
    pub id: i64,
    pub at: String,
    pub entity: String,
    pub entity_id: Option<i64>,
    pub action: String,
    pub summary: String,
    pub detail: Option<String>,
}

pub fn audit_list(conn: &Connection, entity: Option<&str>, before_id: Option<i64>, limit: i64) -> AppResult<Vec<AuditEntry>> {
    let mut st = conn.prepare(
        "SELECT id, at, entity, entity_id, action, summary, detail FROM audit_log
         WHERE (?1 IS NULL OR entity = ?1) AND (?2 IS NULL OR id < ?2) ORDER BY id DESC LIMIT ?3",
    )?;
    let rows = st.query_map(params![entity, before_id, limit.clamp(1, 500)], |r| {
        Ok(AuditEntry {
            id: r.get(0)?,
            at: r.get(1)?,
            entity: r.get(2)?,
            entity_id: r.get(3)?,
            action: r.get(4)?,
            summary: r.get(5)?,
            detail: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;

    #[test]
    fn business_defaults_and_validation() {
        let conn = open_memory();
        let mut b = get_business(&conn).unwrap();
        assert!(!b.onboarding_completed);
        assert!(save_business(&conn, &b).is_err()); // name required
        b.name = "Studio".into();
        b.schedule[0].end = "08:00".into();
        assert!(save_business(&conn, &b).is_err());
        b.schedule[0].end = "17:00".into();
        save_business(&conn, &b).unwrap();
        assert_eq!(get_business(&conn).unwrap().name, "Studio");
    }

    #[test]
    fn location_validation_and_archive() {
        let conn = open_memory();
        let mut l = Location {
            id: None,
            name: "Main".into(),
            address_line: "1 Main St".into(),
            city: "Tumwater".into(),
            state: "wa".into(),
            postal_code: "98501".into(),
            latitude: None,
            longitude: None,
            geo_precision: None,
            geo_source: None,
            state_fips: None,
            county_fips: None,
            place_fips: None,
            archived: false,
        };
        let id = save_location(&conn, &l).unwrap();
        assert_eq!(location(&conn, id).unwrap().state, "WA");
        l.postal_code = "9850".into();
        assert!(save_location(&conn, &l).is_err());
        set_archived(&conn, "locations", "location", id, true).unwrap();
        assert!(location(&conn, id).unwrap().archived);
        assert!(audit_list(&conn, Some("location"), None, 10).unwrap().len() >= 2);
    }
}
