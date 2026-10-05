//! Saved dashboard layouts ("views"). A layout is a list of widgets with grid positions and
//! optional per-widget filters; it's stored as JSON and validated for shape and size.

use super::{audit, now_utc};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Widget {
    pub i: String,
    pub kind: String,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    /// Widget-level filters override the dashboard's for the same field.
    #[serde(default)]
    pub filters: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Dashboard {
    pub id: Option<i64>,
    pub name: String,
    pub layout: Vec<Widget>,
    pub filters: serde_json::Map<String, serde_json::Value>,
    pub is_default: bool,
}

pub const KINDS: &[&str] = &[
    "kpi_revenue", "kpi_profit", "money_in", "trend", "by_service", "by_profile", "by_staff", "costs", "inventory",
    "low_stock", "estimate_vs_actual", "upcoming", "pricing_alerts", "market", "operating",
];

fn default_layout() -> Vec<Widget> {
    let w = |i: &str, kind: &str, x, y, w, h| Widget { i: i.into(), kind: kind.into(), x, y, w, h, filters: Default::default() };
    vec![
        w("a", "kpi_revenue", 0, 0, 3, 2),
        w("b", "kpi_profit", 3, 0, 3, 2),
        w("c", "money_in", 6, 0, 6, 4),
        w("d", "trend", 0, 2, 6, 5),
        w("e", "operating", 6, 4, 6, 3),
        w("f", "by_service", 0, 7, 8, 5),
        w("g", "upcoming", 8, 7, 4, 5),
        w("h", "costs", 0, 12, 4, 4),
        w("i", "inventory", 4, 12, 4, 4),
        w("j", "pricing_alerts", 8, 12, 4, 4),
        w("k", "estimate_vs_actual", 0, 16, 6, 4),
        w("l", "market", 6, 16, 6, 4),
    ]
}

pub fn list(conn: &Connection) -> AppResult<Vec<Dashboard>> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM dashboards", [], |r| r.get(0))?;
    if n == 0 {
        save(conn, &Dashboard { id: None, name: "Overview".into(), layout: default_layout(), filters: Default::default(), is_default: true })?;
    }
    let mut st = conn.prepare("SELECT id, name, layout, filters, is_default FROM dashboards ORDER BY is_default DESC, name")?;
    let rows: Vec<(i64, String, String, String, i64)> = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<Result<_, _>>()?;
    rows.into_iter()
        .map(|(id, name, layout, filters, d)| Ok(Dashboard { id: Some(id), name, layout: serde_json::from_str(&layout)?, filters: serde_json::from_str(&filters)?, is_default: d != 0 }))
        .collect()
}

pub fn save(conn: &Connection, d: &Dashboard) -> AppResult<i64> {
    if d.name.trim().is_empty() {
        return Err(AppError::invalid("name", "Name the view."));
    }
    if d.layout.len() > 40 {
        return Err(AppError::msg("A dashboard can hold up to 40 widgets."));
    }
    for w in &d.layout {
        if !KINDS.contains(&w.kind.as_str()) {
            return Err(AppError::msg(format!("Unknown widget type {}", w.kind)));
        }
        if w.w < 1 || w.h < 1 || w.x < 0 || w.y < 0 || w.w > 12 || w.x + w.w > 12 {
            return Err(AppError::msg("Widget position is out of range."));
        }
    }
    let layout = serde_json::to_string(&d.layout)?;
    let filters = serde_json::to_string(&d.filters)?;
    if d.is_default {
        conn.execute("UPDATE dashboards SET is_default = 0", [])?;
    }
    let id = match d.id {
        Some(id) => {
            conn.execute("UPDATE dashboards SET name=?1, layout=?2, filters=?3, is_default=?4, updated_at=?5 WHERE id=?6", params![d.name.trim(), layout, filters, d.is_default as i64, now_utc(), id])?;
            id
        }
        None => {
            conn.execute("INSERT INTO dashboards (name, layout, filters, is_default) VALUES (?1,?2,?3,?4)", params![d.name.trim(), layout, filters, d.is_default as i64])?;
            conn.last_insert_rowid()
        }
    };
    audit(conn, "dashboard", Some(id), "save", &format!("Dashboard view \"{}\" saved", d.name.trim()), None)?;
    Ok(id)
}

pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM dashboards", [], |r| r.get(0))?;
    if n <= 1 {
        return Err(AppError::msg("Keep at least one dashboard view."));
    }
    conn.execute("DELETE FROM dashboards WHERE id = ?1", [id])?;
    conn.execute("UPDATE dashboards SET is_default = 1 WHERE id = (SELECT MIN(id) FROM dashboards) AND NOT EXISTS (SELECT 1 FROM dashboards WHERE is_default = 1)", [])?;
    Ok(())
}
