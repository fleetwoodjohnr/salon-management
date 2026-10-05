//! Clients, their service history, and versioned formulas (e.g. color recipes).

use super::{audit, now_utc};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ClientInput {
    pub id: Option<i64>,
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
    pub email: String,
    pub sensitivities: String,
    pub notes: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct ClientRow {
    pub client: ClientInput,
    pub archived: bool,
    pub visits: i64,
    pub last_visit: Option<String>,
    pub next_appointment: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FormulaLine {
    pub product_id: Option<i64>,
    pub product_name: String,
    pub qty: Decimal,
    pub unit: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct FormulaVersion {
    pub id: i64,
    pub version: i64,
    pub body: String,
    pub lines: Vec<FormulaLine>,
    pub note: String,
    pub sale_id: Option<i64>,
    pub created_at: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Formula {
    pub id: i64,
    pub title: String,
    pub service_id: Option<i64>,
    pub service_name: Option<String>,
    pub versions: Vec<FormulaVersion>,
}

#[derive(Serialize, Clone, Debug)]
pub struct HistoryItem {
    pub sale_id: i64,
    pub number: Option<String>,
    pub sale_date: String,
    pub status: String,
    pub description: String,
    pub staff_name: Option<String>,
    pub net: Option<Decimal>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ClientDetail {
    pub row: ClientRow,
    pub history: Vec<HistoryItem>,
    pub formulas: Vec<Formula>,
}

fn validate(c: &ClientInput) -> AppResult<()> {
    if c.first_name.trim().is_empty() {
        return Err(AppError::invalid("first_name", "Enter at least a first name."));
    }
    let e = c.email.trim();
    if !e.is_empty() && (!e.contains('@') || e.contains(' ')) {
        return Err(AppError::invalid("email", "That email address doesn't look right."));
    }
    Ok(())
}

pub fn save(conn: &Connection, c: &ClientInput) -> AppResult<i64> {
    validate(c)?;
    let p = params![c.first_name.trim(), c.last_name.trim(), c.phone.trim(), c.email.trim(), c.sensitivities.trim(), c.notes, now_utc(), c.id];
    let id = match c.id {
        Some(id) => {
            conn.execute("UPDATE clients SET first_name=?1, last_name=?2, phone=?3, email=?4, sensitivities=?5, notes=?6, updated_at=?7 WHERE id=?8", p)?;
            id
        }
        None => {
            conn.execute("INSERT INTO clients (first_name, last_name, phone, email, sensitivities, notes, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7)", &p[..7])?;
            conn.last_insert_rowid()
        }
    };
    audit(conn, "client", Some(id), if c.id.is_some() { "update" } else { "create" }, &format!("Client {} {} saved", c.first_name.trim(), c.last_name.trim()), None)?;
    Ok(id)
}

const ROW_SQL: &str = "SELECT c.id, c.first_name, c.last_name, c.phone, c.email, c.sensitivities, c.notes, c.archived_at,
    (SELECT COUNT(*) FROM sales s WHERE s.client_id = c.id AND s.status = 'finalized'),
    (SELECT MAX(s.sale_date) FROM sales s WHERE s.client_id = c.id AND s.status = 'finalized'),
    (SELECT MIN(a.starts_at) FROM appointments a WHERE a.client_id = c.id AND a.status IN ('scheduled','checked_in') AND a.starts_at >= ?1)
    FROM clients c";

fn client_row(r: &rusqlite::Row) -> rusqlite::Result<ClientRow> {
    Ok(ClientRow {
        client: ClientInput {
            id: r.get(0)?,
            first_name: r.get(1)?,
            last_name: r.get(2)?,
            phone: r.get(3)?,
            email: r.get(4)?,
            sensitivities: r.get(5)?,
            notes: r.get(6)?,
        },
        archived: r.get::<_, Option<String>>(7)?.is_some(),
        visits: r.get(8)?,
        last_visit: r.get(9)?,
        next_appointment: r.get(10)?,
    })
}

pub fn list(conn: &Connection, include_archived: bool, now_local: &str) -> AppResult<Vec<ClientRow>> {
    let mut st = conn.prepare(&format!("{ROW_SQL} WHERE (?2 OR c.archived_at IS NULL) ORDER BY c.last_name COLLATE NOCASE, c.first_name COLLATE NOCASE"))?;
    let rows = st.query_map(params![now_local, include_archived], client_row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn detail(conn: &Connection, id: i64, now_local: &str) -> AppResult<ClientDetail> {
    let row = conn
        .query_row(&format!("{ROW_SQL} WHERE c.id = ?2"), params![now_local, id], client_row)
        .optional()?
        .ok_or_else(|| AppError::NotFound("Client not found.".into()))?;
    let mut st = conn.prepare(
        "SELECT s.id, s.number, s.sale_date, s.status, sl.description, st.name, sl.net FROM sale_lines sl JOIN sales s ON s.id = sl.sale_id
         LEFT JOIN staff st ON st.id = sl.staff_id WHERE s.client_id = ?1 AND sl.kind <> 'tip' ORDER BY s.sale_date DESC, s.id DESC, sl.sort",
    )?;
    let history = st
        .query_map([id], |r| {
            Ok(HistoryItem {
                sale_id: r.get(0)?,
                number: r.get(1)?,
                sale_date: r.get(2)?,
                status: r.get(3)?,
                description: r.get(4)?,
                staff_name: r.get(5)?,
                net: super::opt_dec(r, "net")?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(ClientDetail { row, history, formulas: formulas(conn, id)? })
}

pub fn formulas(conn: &Connection, client_id: i64) -> AppResult<Vec<Formula>> {
    let mut st = conn.prepare(
        "SELECT f.id, f.title, f.service_id, s.name FROM client_formulas f LEFT JOIN services s ON s.id = f.service_id WHERE f.client_id = ?1 AND f.archived_at IS NULL ORDER BY f.id DESC",
    )?;
    let heads: Vec<(i64, String, Option<i64>, Option<String>)> = st.query_map([client_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<Result<_, _>>()?;
    let mut vst = conn.prepare("SELECT id, version, body, lines, note, sale_id, created_at FROM client_formula_versions WHERE formula_id = ?1 ORDER BY version DESC")?;
    heads
        .into_iter()
        .map(|(id, title, service_id, service_name)| {
            let versions = vst
                .query_map([id], |r| {
                    Ok(FormulaVersion {
                        id: r.get(0)?,
                        version: r.get(1)?,
                        body: r.get(2)?,
                        lines: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
                        note: r.get(4)?,
                        sale_id: r.get(5)?,
                        created_at: r.get(6)?,
                    })
                })?
                .collect::<Result<_, _>>()?;
            Ok(Formula { id, title, service_id, service_name, versions })
        })
        .collect()
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct FormulaInput {
    /// None = new formula; Some = add a version to it.
    pub formula_id: Option<i64>,
    pub client_id: i64,
    pub title: String,
    pub service_id: Option<i64>,
    pub body: String,
    pub lines: Vec<FormulaLine>,
    pub note: String,
    pub sale_id: Option<i64>,
}

pub fn save_formula(conn: &Connection, f: &FormulaInput) -> AppResult<i64> {
    if f.body.trim().is_empty() && f.lines.is_empty() {
        return Err(AppError::invalid("body", "Write the formula or add products."));
    }
    for l in &f.lines {
        if l.qty <= Decimal::ZERO {
            return Err(AppError::invalid("lines", "Amounts must be more than zero."));
        }
    }
    let id = match f.formula_id {
        Some(id) => id,
        None => {
            if f.title.trim().is_empty() {
                return Err(AppError::invalid("title", "Name the formula, e.g. Root color."));
            }
            conn.execute("INSERT INTO client_formulas (client_id, title, service_id) VALUES (?1, ?2, ?3)", params![f.client_id, f.title.trim(), f.service_id])?;
            conn.last_insert_rowid()
        }
    };
    let next: i64 = conn.query_row("SELECT COALESCE(MAX(version), 0) + 1 FROM client_formula_versions WHERE formula_id = ?1", [id], |r| r.get(0))?;
    conn.execute(
        "INSERT INTO client_formula_versions (formula_id, version, body, lines, note, sale_id) VALUES (?1,?2,?3,?4,?5,?6)",
        params![id, next, f.body.trim(), serde_json::to_string(&f.lines)?, f.note.trim(), f.sale_id],
    )?;
    audit(conn, "client_formula", Some(id), "version", &format!("Formula saved as version {next}"), None)?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;
    use rust_decimal_macros::dec;

    #[test]
    fn clients_and_formula_versions() {
        let conn = open_memory();
        let c = ClientInput { id: None, first_name: "Mia".into(), last_name: "Ortiz".into(), phone: String::new(), email: "mia@example.com".into(), sensitivities: "PPD patch test".into(), notes: String::new() };
        let id = save(&conn, &c).unwrap();
        assert!(save(&conn, &ClientInput { email: "bad".into(), ..c.clone() }).is_err());
        let f = save_formula(
            &conn,
            &FormulaInput { formula_id: None, client_id: id, title: "Root".into(), service_id: None, body: "7N + 20 vol 1:1".into(), lines: vec![FormulaLine { product_id: None, product_name: "7N".into(), qty: dec!(30), unit: "g".into() }], note: String::new(), sale_id: None },
        )
        .unwrap();
        save_formula(&conn, &FormulaInput { formula_id: Some(f), client_id: id, title: String::new(), service_id: None, body: "7N + 6N 1:1, 20 vol".into(), lines: vec![], note: "Warmer".into(), sale_id: None }).unwrap();
        let d = detail(&conn, id, "2026-10-01T00:00").unwrap();
        assert_eq!(d.formulas[0].versions.len(), 2);
        assert_eq!(d.formulas[0].versions[1].body, "7N + 20 vol 1:1"); // first version kept
        assert_eq!(list(&conn, false, "2026-10-01T00:00").unwrap().len(), 1);
    }
}
