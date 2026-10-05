//! Estimates (quotes) a client can take away or print; convert into an appointment when booked.

use super::{audit, dec};
use super::services;
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct EstimateLine {
    pub service_id: i64,
    pub variant_ids: Vec<i64>,
    pub qty: i64,
    pub unit_price: Decimal,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct EstimateInput {
    pub id: Option<i64>,
    pub client_id: Option<i64>,
    pub staff_id: Option<i64>,
    pub location_id: Option<i64>,
    pub issued_on: String,
    pub valid_until: Option<String>,
    pub notes: String,
    pub lines: Vec<EstimateLine>,
}

#[derive(Serialize, Clone, Debug)]
pub struct EstimateView {
    pub input: EstimateInput,
    pub status: String,
    pub client_name: Option<String>,
    pub subtotal: Decimal,
}

/// Default price for an estimate line: the service price plus its variants' price adjustments.
pub fn default_price(conn: &Connection, service_id: i64, variant_ids: &[i64]) -> AppResult<Decimal> {
    let s = services::get(conn, service_id)?;
    let (_, _, delta, _) = services::apply_variants(&s.input, variant_ids)?;
    Ok(s.input.price.unwrap_or_default() + delta)
}

pub fn save(conn: &Connection, e: &EstimateInput) -> AppResult<i64> {
    crate::db::inventory::valid_date("issued_on", &e.issued_on)?;
    if let Some(v) = &e.valid_until {
        crate::db::inventory::valid_date("valid_until", v)?;
    }
    if e.lines.is_empty() {
        return Err(AppError::invalid("lines", "Add at least one service."));
    }
    for l in &e.lines {
        if l.qty < 1 || l.unit_price < Decimal::ZERO || l.unit_price != l.unit_price.round_dp(2) {
            return Err(AppError::invalid("lines", "Check quantities and prices."));
        }
        services::get(conn, l.service_id)?;
    }
    let id = match e.id {
        Some(id) => {
            conn.execute(
                "UPDATE estimates SET client_id=?1, staff_id=?2, location_id=?3, issued_on=?4, valid_until=?5, notes=?6 WHERE id=?7 AND status = 'open'",
                params![e.client_id, e.staff_id, e.location_id, e.issued_on, e.valid_until, e.notes, id],
            )?;
            conn.execute("DELETE FROM estimate_lines WHERE estimate_id = ?1", [id])?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO estimates (client_id, staff_id, location_id, issued_on, valid_until, notes) VALUES (?1,?2,?3,?4,?5,?6)",
                params![e.client_id, e.staff_id, e.location_id, e.issued_on, e.valid_until, e.notes],
            )?;
            conn.last_insert_rowid()
        }
    };
    for (i, l) in e.lines.iter().enumerate() {
        conn.execute(
            "INSERT INTO estimate_lines (estimate_id, service_id, variant_ids, qty, unit_price, description, sort) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![id, l.service_id, serde_json::to_string(&l.variant_ids)?, l.qty, l.unit_price.to_string(), l.description, i as i64],
        )?;
    }
    audit(conn, "estimate", Some(id), "save", "Estimate saved", None)?;
    Ok(id)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<EstimateView> {
    let (input, status, client_name) = conn
        .query_row(
            "SELECT e.client_id, e.staff_id, e.location_id, e.issued_on, e.valid_until, e.notes, e.status, TRIM(c.first_name || ' ' || c.last_name)
             FROM estimates e LEFT JOIN clients c ON c.id = e.client_id WHERE e.id = ?1",
            [id],
            |r| {
                Ok((
                    EstimateInput { id: Some(id), client_id: r.get(0)?, staff_id: r.get(1)?, location_id: r.get(2)?, issued_on: r.get(3)?, valid_until: r.get(4)?, notes: r.get(5)?, lines: vec![] },
                    r.get::<_, String>(6)?,
                    r.get::<_, Option<String>>(7)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Estimate not found.".into()))?;
    let mut input = input;
    let mut st = conn.prepare("SELECT service_id, variant_ids, qty, unit_price, description FROM estimate_lines WHERE estimate_id = ?1 ORDER BY sort")?;
    input.lines = st
        .query_map([id], |r| {
            Ok(EstimateLine { service_id: r.get(0)?, variant_ids: serde_json::from_str(&r.get::<_, String>(1)?).unwrap_or_default(), qty: r.get(2)?, unit_price: dec(r, "unit_price")?, description: r.get(4)? })
        })?
        .collect::<Result<_, _>>()?;
    let subtotal = input.lines.iter().map(|l| l.unit_price * Decimal::from(l.qty)).sum();
    Ok(EstimateView { input, status, client_name, subtotal })
}

pub fn list(conn: &Connection) -> AppResult<Vec<EstimateView>> {
    let ids: Vec<i64> = {
        let mut st = conn.prepare("SELECT id FROM estimates ORDER BY issued_on DESC, id DESC LIMIT 500")?;
        let rows = st.query_map([], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    ids.into_iter().map(|id| get(conn, id)).collect()
}

pub fn set_status(conn: &Connection, id: i64, status: &str) -> AppResult<()> {
    if !matches!(status, "open" | "accepted" | "declined") {
        return Err(AppError::invalid("status", "Unknown status."));
    }
    conn.execute("UPDATE estimates SET status = ?1 WHERE id = ?2", params![status, id])?;
    Ok(())
}
