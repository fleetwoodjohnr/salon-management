//! Business expenses (period spending). Inventory purchases are recorded in Inventory, not here,
//! so reports can show cash spent on stock separately from stock actually used.

use super::{audit, dec, now_utc};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub const CATEGORIES: &[(&str, &str)] = &[
    ("rent", "Rent or chair rental"),
    ("utilities", "Utilities"),
    ("insurance", "Insurance"),
    ("software", "Software and subscriptions"),
    ("supplies", "Supplies not tracked in inventory"),
    ("equipment", "Equipment and tools"),
    ("education", "Education and licensing"),
    ("marketing", "Marketing"),
    ("fees", "Bank and processing fees"),
    ("payroll", "Wages and payroll"),
    ("other", "Other"),
];

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ExpenseInput {
    pub id: Option<i64>,
    pub expense_date: String,
    pub category: String,
    pub vendor: String,
    pub description: String,
    pub amount: Decimal,
    pub payment_method: String,
    pub location_id: Option<i64>,
    pub attachment_id: Option<i64>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Expense {
    pub input: ExpenseInput,
    pub attachment_name: Option<String>,
    pub voided: bool,
    pub void_reason: Option<String>,
}

pub fn save(conn: &Connection, e: &ExpenseInput) -> AppResult<i64> {
    crate::db::inventory::valid_date("expense_date", &e.expense_date)?;
    if !CATEGORIES.iter().any(|(c, _)| *c == e.category) {
        return Err(AppError::invalid("category", "Choose a category."));
    }
    if e.amount <= Decimal::ZERO || e.amount != e.amount.round_dp(2) {
        return Err(AppError::invalid("amount", "Enter an amount in dollars and cents."));
    }
    let p = params![e.expense_date, e.category, e.vendor.trim(), e.description.trim(), e.amount.to_string(), e.payment_method, e.location_id, e.attachment_id, e.id];
    let id = match e.id {
        Some(id) => {
            let n = conn.execute(
                "UPDATE expenses SET expense_date=?1, category=?2, vendor=?3, description=?4, amount=?5, payment_method=?6, location_id=?7, attachment_id=?8 WHERE id=?9 AND voided_at IS NULL",
                p,
            )?;
            if n == 0 {
                return Err(AppError::msg("Voided expenses can't be edited."));
            }
            id
        }
        None => {
            conn.execute(
                "INSERT INTO expenses (expense_date, category, vendor, description, amount, payment_method, location_id, attachment_id) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                &p[..8],
            )?;
            conn.last_insert_rowid()
        }
    };
    audit(conn, "expense", Some(id), if e.id.is_some() { "update" } else { "create" }, &format!("Expense ${} ({}) saved", e.amount, e.category), Some(serde_json::to_value(e)?))?;
    Ok(id)
}

pub fn void(conn: &Connection, id: i64, reason: &str) -> AppResult<()> {
    let n = conn.execute("UPDATE expenses SET voided_at=?1, void_reason=?2 WHERE id=?3 AND voided_at IS NULL", params![now_utc(), reason.trim(), id])?;
    if n == 0 {
        return Err(AppError::msg("Expense not found or already voided."));
    }
    audit(conn, "expense", Some(id), "void", &format!("Expense voided: {}", reason.trim()), None)?;
    Ok(())
}

pub fn list(conn: &Connection, from: Option<&str>, to: Option<&str>) -> AppResult<Vec<Expense>> {
    let mut st = conn.prepare(
        "SELECT e.id, e.expense_date, e.category, e.vendor, e.description, e.amount, e.payment_method, e.location_id, e.attachment_id, a.file_name, e.voided_at, e.void_reason
         FROM expenses e LEFT JOIN attachments a ON a.id = e.attachment_id WHERE (?1 IS NULL OR e.expense_date >= ?1) AND (?2 IS NULL OR e.expense_date <= ?2)
         ORDER BY e.expense_date DESC, e.id DESC",
    )?;
    let rows = st.query_map(params![from, to], |r| {
        Ok(Expense {
            input: ExpenseInput {
                id: r.get(0)?,
                expense_date: r.get(1)?,
                category: r.get(2)?,
                vendor: r.get(3)?,
                description: r.get(4)?,
                amount: dec(r, "amount")?,
                payment_method: r.get(6)?,
                location_id: r.get(7)?,
                attachment_id: r.get(8)?,
            },
            attachment_name: r.get(9)?,
            voided: r.get::<_, Option<String>>(10)?.is_some(),
            void_reason: r.get(11)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
