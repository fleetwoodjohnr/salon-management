//! SQLite access: connection setup, migrations, and small row helpers.
//! Each submodule owns the SQL for one area.

pub mod appointments;
pub mod clients;
pub mod core;
pub mod dashboards;
pub mod estimates;
pub mod expenses;
pub mod inventory;
pub mod market;
pub mod pricing;
pub mod profiles;
pub mod reports;
pub mod research;
pub mod sales;
pub mod services;
pub mod tax;

use crate::error::{AppError, AppResult};
use rusqlite::{Connection, Row};
use rust_decimal::Decimal;
use std::path::Path;

pub const MIGRATIONS: &[&str] = &[
    include_str!("../../migrations/001_core.sql"),
    include_str!("../../migrations/002_inventory_services.sql"),
    include_str!("../../migrations/003_tax.sql"),
    include_str!("../../migrations/004_providers.sql"),
    include_str!("../../migrations/005_sales.sql"),
    include_str!("../../migrations/006_market_dashboards.sql"),
    include_str!("../../migrations/007_market_research.sql"),
];

pub fn schema_version() -> i64 {
    MIGRATIONS.len() as i64
}

pub fn configure(conn: &Connection) -> AppResult<()> {
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    // WAL for crash safety; FULL sync because this is financial data.
    conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
    conn.pragma_update(None, "synchronous", "FULL")?;
    Ok(())
}

pub fn user_version(conn: &Connection) -> AppResult<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |r| r.get(0))?)
}

/// Apply pending migrations, each in its own transaction. `before_migrate` runs once (with the
/// current version) when an existing database needs upgrading — used to take a backup first.
pub fn migrate(conn: &mut Connection, before_migrate: impl FnOnce(&Connection, i64) -> AppResult<()>) -> AppResult<()> {
    let current = user_version(conn)?;
    let target = schema_version();
    if current > target {
        return Err(AppError::Other(format!(
            "This workspace was saved by a newer version of Salon Resource Manager (data version {current}, this app supports {target}). Update the app to open it."
        )));
    }
    if current == target {
        return Ok(());
    }
    if current > 0 {
        before_migrate(conn, current)?;
    }
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let v = i as i64 + 1;
        if v <= current {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", v)?;
        tx.commit()?;
    }
    let bad: Vec<String> = {
        let mut st = conn.prepare("PRAGMA foreign_key_check")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        rows.collect::<Result<_, _>>()?
    };
    if !bad.is_empty() {
        return Err(AppError::Other(format!("Foreign-key check failed after migration in: {}", bad.join(", "))));
    }
    Ok(())
}

pub fn open(path: &Path, before_migrate: impl FnOnce(&Connection, i64) -> AppResult<()>) -> AppResult<Connection> {
    let mut conn = Connection::open(path)?;
    configure(&conn)?;
    migrate(&mut conn, before_migrate)?;
    Ok(conn)
}

#[cfg(test)]
pub fn open_memory() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    migrate(&mut conn, |_, _| Ok(())).unwrap();
    conn
}

pub fn now_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Read a TEXT decimal column.
pub fn dec(row: &Row, idx: &str) -> rusqlite::Result<Decimal> {
    let s: String = row.get(idx)?;
    s.parse::<Decimal>()
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))
}

pub fn opt_dec(row: &Row, idx: &str) -> rusqlite::Result<Option<Decimal>> {
    let s: Option<String> = row.get(idx)?;
    s.map(|s| {
        s.parse::<Decimal>()
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))
    })
    .transpose()
}

/// Append an audit-trail entry. Call inside the same transaction as the change it describes.
pub fn audit(
    conn: &Connection,
    entity: &str,
    entity_id: Option<i64>,
    action: &str,
    summary: &str,
    detail: Option<serde_json::Value>,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO audit_log (entity, entity_id, action, summary, detail) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![entity, entity_id, action, summary, detail.map(|d| d.to_string())],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_empty_db_to_latest() {
        let conn = open_memory();
        assert_eq!(user_version(&conn).unwrap(), schema_version());
    }

    #[test]
    fn rejects_newer_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", schema_version() + 1).unwrap();
        assert!(migrate(&mut conn, |_, _| Ok(())).is_err());
    }

    #[test]
    fn upgrades_each_prior_version_and_calls_backup_hook() {
        for start in 1..schema_version() {
            let mut conn = Connection::open_in_memory().unwrap();
            conn.pragma_update(None, "foreign_keys", true).unwrap();
            for (i, sql) in MIGRATIONS.iter().take(start as usize).enumerate() {
                conn.execute_batch(sql).unwrap();
                conn.pragma_update(None, "user_version", i as i64 + 1).unwrap();
            }
            let mut called = None;
            migrate(&mut conn, |_, v| {
                called = Some(v);
                Ok(())
            })
            .unwrap();
            assert_eq!(called, Some(start));
            assert_eq!(user_version(&conn).unwrap(), schema_version());
        }
    }


    /// Upgrading a 0.1.0 workspace (data version 6) takes a backup first and keeps every observed
    /// price exactly as it was.
    #[test]
    fn migration_7_keeps_observations() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        for sql in &MIGRATIONS[..6] {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", 6).unwrap();
        conn.execute_batch(
            "INSERT INTO services (id, name, hands_on_min) VALUES (1, 'Cut', 45);
             INSERT INTO competitors (id, name, source, source_ref) VALUES (1, 'A', 'osm', 'node/1');
             INSERT INTO market_observations (id, competitor_id, service_id, service_label, price, price_type, price_max, duration_min, hair_length, source_type, source_url, observed_on, notes, excluded)
               VALUES (1, 1, 1, 'Cut', '45', 'exact', NULL, 50, 'short', 'user_observed', 'https://a.example', '2026-09-01', 'n', 1),
                      (2, 1, NULL, 'Color', '80', 'range', '120', NULL, '', 'csv_import', '', '2026-09-02', '', 0);",
        )
        .unwrap();
        let dump = |c: &Connection| -> Vec<String> {
            let mut st = c
                .prepare("SELECT id, competitor_id, service_id, service_label, price, price_type, price_max, duration_min, hair_length, stylist_level, inclusions, source_type, source_url, observed_on, notes, excluded, created_at FROM market_observations ORDER BY id")
                .unwrap();
            let rows = st.query_map([], |r| Ok((0..17).map(|i| format!("{:?}", r.get::<_, rusqlite::types::Value>(i).unwrap())).collect::<Vec<_>>().join("|"))).unwrap();
            rows.map(Result::unwrap).collect()
        };
        let before = dump(&conn);
        let mut backed_up_from = None;
        migrate(&mut conn, |_, v| {
            backed_up_from = Some(v);
            Ok(())
        })
        .unwrap();
        assert_eq!(backed_up_from, Some(6));
        assert_eq!(user_version(&conn).unwrap(), schema_version());
        assert_eq!(dump(&conn), before);
        let methods: Vec<String> = conn.prepare("SELECT match_method FROM market_observations ORDER BY id").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
        assert_eq!(methods, ["user", "none"]);
        conn.execute("INSERT INTO market_observations (competitor_id, service_label, price, price_type, source_type, observed_on, retrieved_at) VALUES (1, 'Cut', '50', 'exact', 'web_extracted', '2026-10-11', '2026-10-11T10:00:00Z')", []).unwrap();
    }
}
