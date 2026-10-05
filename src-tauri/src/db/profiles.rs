//! Work profiles. Saving appends a new version; history is never rewritten.

use super::audit;
use crate::domain::profile::{ProfileData, ProfileKind, ProfileRates};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct ProfileView {
    pub id: i64,
    pub name: String,
    pub archived: bool,
    pub version: i64,
    pub version_id: i64,
    pub version_created_at: String,
    pub data: ProfileData,
    pub rates: Option<ProfileRates>,
    pub rates_error: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ProfileVersionInfo {
    pub version: i64,
    pub version_id: i64,
    pub created_at: String,
}

fn kind_str(k: ProfileKind) -> &'static str {
    match k {
        ProfileKind::Individual => "individual",
        ProfileKind::ChairRenter => "chair_renter",
        ProfileKind::Independent => "independent",
        ProfileKind::Employee => "employee",
    }
}

fn view_from_row(r: &rusqlite::Row) -> rusqlite::Result<(i64, String, bool, i64, i64, String, String)> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get::<_, Option<String>>(2)?.is_some(),
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
    ))
}

fn build_view(t: (i64, String, bool, i64, i64, String, String)) -> AppResult<ProfileView> {
    let data: ProfileData = serde_json::from_str(&t.6)?;
    let (rates, rates_error) = match data.rates() {
        Ok(r) => (Some(r), None),
        Err(e) => (None, Some(e.to_string())),
    };
    Ok(ProfileView {
        id: t.0,
        name: t.1,
        archived: t.2,
        version: t.3,
        version_id: t.4,
        version_created_at: t.5,
        data,
        rates,
        rates_error,
    })
}

const LATEST_SQL: &str = "SELECT p.id, p.name, p.archived_at, v.version, v.id, v.created_at, v.data
    FROM work_profiles p JOIN work_profile_versions v ON v.profile_id = p.id
    WHERE v.version = (SELECT MAX(version) FROM work_profile_versions WHERE profile_id = p.id)";

pub fn list(conn: &Connection, include_archived: bool) -> AppResult<Vec<ProfileView>> {
    let mut st = conn.prepare(&format!(
        "{LATEST_SQL} AND (?1 OR p.archived_at IS NULL) ORDER BY p.archived_at IS NOT NULL, p.name"
    ))?;
    let rows = st.query_map([include_archived], view_from_row)?;
    rows.map(|r| build_view(r?)).collect()
}

pub fn get(conn: &Connection, id: i64) -> AppResult<ProfileView> {
    let t = conn
        .query_row(&format!("{LATEST_SQL} AND p.id = ?1"), [id], view_from_row)
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("Work profile {id} not found.")))?;
    build_view(t)
}

/// A specific historical version (used to show what a past sale was costed with).
pub fn get_version(conn: &Connection, version_id: i64) -> AppResult<ProfileView> {
    let t = conn
        .query_row(
            "SELECT p.id, p.name, p.archived_at, v.version, v.id, v.created_at, v.data
             FROM work_profile_versions v JOIN work_profiles p ON p.id = v.profile_id WHERE v.id = ?1",
            [version_id],
            view_from_row,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("Profile version {version_id} not found.")))?;
    build_view(t)
}

pub fn versions(conn: &Connection, id: i64) -> AppResult<Vec<ProfileVersionInfo>> {
    let mut st = conn.prepare(
        "SELECT version, id, created_at FROM work_profile_versions WHERE profile_id = ?1 ORDER BY version DESC",
    )?;
    let rows = st.query_map([id], |r| Ok(ProfileVersionInfo { version: r.get(0)?, version_id: r.get(1)?, created_at: r.get(2)? }))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Create a profile or append a new version. Must run inside a transaction.
pub fn save(conn: &Connection, id: Option<i64>, name: &str, data: &ProfileData) -> AppResult<i64> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::invalid("name", "Give the profile a name, e.g. \"Owner – color specialist\"."));
    }
    data.validate()?;
    if let Some(loc) = data.location_id {
        super::core::location(conn, loc)?;
    }
    let json = serde_json::to_string(data)?;
    let (id, version) = match id {
        Some(id) => {
            let current = get(conn, id)?;
            if current.name == name && current.data == *data {
                return Ok(id); // nothing changed: don't create an empty version
            }
            conn.execute("UPDATE work_profiles SET name = ?1 WHERE id = ?2", params![name, id])?;
            (id, current.version + 1)
        }
        None => {
            conn.execute("INSERT INTO work_profiles (name) VALUES (?1)", [name])?;
            (conn.last_insert_rowid(), 1)
        }
    };
    conn.execute(
        "INSERT INTO work_profile_versions (profile_id, version, kind, location_id, data) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, version, kind_str(data.kind), data.location_id, json],
    )?;
    audit(
        conn,
        "work_profile",
        Some(id),
        if version == 1 { "create" } else { "update" },
        &format!("Work profile \"{name}\" saved as version {version}"),
        Some(serde_json::to_value(data)?),
    )?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;
    use crate::domain::profile::sample;
    use rust_decimal_macros::dec;

    #[test]
    fn edits_append_versions_and_keep_history() {
        let conn = open_memory();
        let mut d = sample();
        let id = save(&conn, None, "Owner", &d).unwrap();
        let v1 = get(&conn, id).unwrap();
        assert_eq!(v1.version, 1);
        save(&conn, Some(id), "Owner", &d).unwrap(); // unchanged → no new version
        assert_eq!(versions(&conn, id).unwrap().len(), 1);
        d.overhead.rent = dec!(1500);
        save(&conn, Some(id), "Owner", &d).unwrap();
        let v2 = get(&conn, id).unwrap();
        assert_eq!(v2.version, 2);
        assert_eq!(v2.data.overhead.rent, dec!(1500));
        // the original version is untouched
        assert_eq!(get_version(&conn, v1.version_id).unwrap().data.overhead.rent, dec!(1200));
    }

    #[test]
    fn invalid_profile_not_saved() {
        let conn = open_memory();
        let mut d = sample();
        d.utilization_pct = dec!(0);
        assert!(save(&conn, None, "X", &d).is_err());
        assert!(list(&conn, true).unwrap().is_empty());
    }
}
