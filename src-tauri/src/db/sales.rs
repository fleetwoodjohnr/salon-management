//! Sales: drafts (editable, no stock effect) → finalize (one transaction: deduct actual usage once,
//! snapshot costs and tax) → payments, refunds, or void.

use super::core::{get_business, staff_list};
use super::inventory::{post, product, Op, PostCtx};
use super::services::{self, material_lines};
use super::{audit, dec, now_utc, opt_dec};
use crate::db::tax::{self, current_rate};
use crate::domain::costing::hours;
use crate::domain::money::{frac, round_money};
use crate::domain::sale::{compute, refund_share, LineIn, LineKind, SaleOut, Taxability};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct UsageInput {
    pub product_id: i64,
    pub planned_qty: Decimal,
    pub qty: Decimal,
    pub unit: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SaleLineInput {
    pub kind: LineKind,
    pub service_id: Option<i64>,
    pub product_id: Option<i64>,
    pub staff_id: Option<i64>,
    pub description: String,
    pub variant_ids: Vec<i64>,
    pub qty: Decimal,
    pub unit_price: Decimal,
    pub line_discount: Decimal,
    pub planned_minutes: Option<i64>,
    pub actual_minutes: Option<i64>,
    pub usage: Vec<UsageInput>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SaleDraft {
    pub id: Option<i64>,
    pub client_id: Option<i64>,
    pub staff_id: Option<i64>,
    pub location_id: Option<i64>,
    pub appointment_id: Option<i64>,
    pub sale_date: String,
    pub sale_discount: Decimal,
    pub notes: String,
    pub lines: Vec<SaleLineInput>,
}

#[derive(Serialize, Clone, Debug)]
pub struct LineSnapshot {
    pub id: i64,
    pub gross: Option<Decimal>,
    pub sale_discount_share: Option<Decimal>,
    pub net: Option<Decimal>,
    pub tax: Option<Decimal>,
    pub taxability: Option<String>,
    pub taxability_basis: Option<String>,
    pub tax_category: String,
    pub materials_cost: Option<Decimal>,
    pub planned_materials_cost: Option<Decimal>,
    pub labor_cost: Option<Decimal>,
    pub overhead_cost: Option<Decimal>,
    pub other_direct_cost: Option<Decimal>,
    pub commission: Option<Decimal>,
    pub retail_cost: Option<Decimal>,
    pub refunded_qty: Decimal,
    pub refunded_net: Decimal,
    pub refunded_tax: Decimal,
    pub usage_costs: Vec<Option<Decimal>>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PaymentInput {
    pub method: String,
    pub amount: Decimal,
    pub reference: String,
    pub paid_on: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Payment {
    pub id: i64,
    pub refund_id: Option<i64>,
    pub method: String,
    pub amount: Decimal,
    pub fee: Decimal,
    pub reference: String,
    pub paid_on: String,
    pub voided: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct RefundView {
    pub id: i64,
    pub number: String,
    pub refund_date: String,
    pub reason: String,
    pub net_total: Decimal,
    pub tax_total: Decimal,
    pub tip_total: Decimal,
    pub total: Decimal,
}

#[derive(Serialize, Clone, Debug)]
pub struct SaleView {
    pub draft: SaleDraft,
    pub number: Option<String>,
    pub status: String,
    pub client_name: Option<String>,
    pub staff_name: Option<String>,
    pub location_name: Option<String>,
    pub created_at: String,
    pub finalized_at: Option<String>,
    pub voided_at: Option<String>,
    pub void_reason: Option<String>,
    /// Live calculation for drafts; the stored snapshot for finalized sales.
    pub totals: SaleOut,
    pub tax_label: Option<String>,
    pub tax_rate: Option<Decimal>,
    pub tax_rate_status: Option<String>,
    pub lines: Vec<LineSnapshot>,
    pub payments: Vec<Payment>,
    pub refunds: Vec<RefundView>,
    pub paid: Decimal,
    pub refunded: Decimal,
    pub balance: Decimal,
    /// Reasons the draft can't be finalized yet (empty = ready).
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
}

pub const PAYMENT_METHODS: &[&str] = &["cash", "card", "check", "transfer", "other"];

// ---------------------------------------------------------------- helpers

fn line_category(conn: &Connection, l: &SaleLineInput) -> AppResult<String> {
    Ok(match l.kind {
        LineKind::Service => services::get(conn, l.service_id.ok_or_else(|| AppError::msg("Service line without a service."))?)?.input.tax_category,
        LineKind::Retail => "retail".into(),
        LineKind::Tip => "tips".into(),
    })
}

struct TaxResolution {
    taxability: Vec<(Taxability, String)>,
    rate_pct: Option<Decimal>,
    rate_set_id: Option<i64>,
    label: Option<String>,
    rate_status: Option<String>,
}

fn resolve_tax(conn: &Connection, location_id: Option<i64>, categories: &[String]) -> AppResult<TaxResolution> {
    let loc = location_id.or(get_business(conn)?.primary_location_id);
    let mut taxability = Vec::new();
    for c in categories {
        let rule = match loc {
            Some(l) => tax::current_rule(conn, l, c)?,
            None => None,
        };
        taxability.push(match rule {
            Some((_, s, basis, _)) if s == "taxable" => (Taxability::Taxable, basis),
            Some((_, s, basis, _)) if s == "exempt" => (Taxability::Exempt, basis),
            _ => (Taxability::Unknown, String::new()),
        });
    }
    let rate = match loc {
        Some(l) => current_rate(conn, l)?,
        None => None,
    };
    Ok(TaxResolution {
        taxability,
        rate_pct: rate.as_ref().map(|r| r.total_rate),
        rate_set_id: rate.as_ref().map(|r| r.id),
        label: rate.as_ref().map(|r| format!("{}% {}", r.total_rate.normalize(), r.jurisdiction_label)),
        rate_status: rate.map(|r| if r.freshness == "stale" { "stale".to_string() } else { r.status }),
    })
}

fn line_ins(d: &SaleDraft, res: &TaxResolution) -> Vec<LineIn> {
    d.lines
        .iter()
        .zip(&res.taxability)
        .map(|(l, (t, _))| LineIn { kind: l.kind, qty: l.qty, unit_price: l.unit_price, line_discount: l.line_discount, taxability: *t, label: l.description.clone() })
        .collect()
}

fn validate_draft(conn: &Connection, d: &SaleDraft) -> AppResult<()> {
    crate::db::inventory::valid_date("sale_date", &d.sale_date)?;
    for (i, l) in d.lines.iter().enumerate() {
        let f = |n: &str| format!("lines.{i}.{n}");
        match l.kind {
            LineKind::Service => {
                services::get(conn, l.service_id.ok_or_else(|| AppError::invalid(&f("service_id"), "Choose a service."))?)?;
            }
            LineKind::Retail => {
                let p = product(conn, l.product_id.ok_or_else(|| AppError::invalid(&f("product_id"), "Choose a product."))?)?;
                if p.category != "retail" {
                    return Err(AppError::invalid(&f("product_id"), format!("{} isn't a retail product.", p.name)));
                }
            }
            LineKind::Tip => {
                if l.qty != Decimal::ONE {
                    return Err(AppError::invalid(&f("qty"), "A tip line has a quantity of 1."));
                }
            }
        }
        if l.unit_price != l.unit_price.round_dp(2) || l.line_discount != l.line_discount.round_dp(2) {
            return Err(AppError::invalid(&f("unit_price"), "Amounts must be in whole cents."));
        }
        if l.kind != LineKind::Service && !l.usage.is_empty() {
            return Err(AppError::invalid(&f("usage"), "Only service lines record product usage."));
        }
        for (j, u) in l.usage.iter().enumerate() {
            if u.qty < Decimal::ZERO || u.planned_qty < Decimal::ZERO {
                return Err(AppError::invalid(&format!("lines.{i}.usage.{j}.qty"), "Usage can't be negative."));
            }
            product(conn, u.product_id)?.units().to_base(u.qty, &u.unit).map_err(|e| AppError::invalid(&format!("lines.{i}.usage.{j}.unit"), e.to_string()))?;
        }
        for m in [l.planned_minutes, l.actual_minutes].into_iter().flatten() {
            if !(0..=24 * 60).contains(&m) {
                return Err(AppError::invalid(&f("actual_minutes"), "Minutes must be between 0 and 1440."));
            }
        }
    }
    if d.sale_discount != d.sale_discount.round_dp(2) {
        return Err(AppError::invalid("sale_discount", "Discount must be in whole cents."));
    }
    Ok(())
}

// ---------------------------------------------------------------- drafts

pub fn save_draft(conn: &Connection, d: &SaleDraft) -> AppResult<i64> {
    validate_draft(conn, d)?;
    // Arithmetic check now so a draft is never saved in an impossible state.
    let cats: Vec<String> = d.lines.iter().map(|l| line_category(conn, l)).collect::<AppResult<_>>()?;
    let res = resolve_tax(conn, d.location_id, &cats)?;
    compute(&line_ins(d, &res), d.sale_discount, res.rate_pct, get_business(conn)?.prices_include_tax)?;
    let include = get_business(conn)?.prices_include_tax as i64;
    let id = match d.id {
        Some(id) => {
            let n = conn.execute(
                "UPDATE sales SET client_id=?1, staff_id=?2, location_id=?3, sale_date=?4, sale_discount=?5, notes=?6, prices_include_tax=?7 WHERE id=?8 AND status='draft'",
                params![d.client_id, d.staff_id, d.location_id, d.sale_date, d.sale_discount.to_string(), d.notes, include, id],
            )?;
            if n == 0 {
                return Err(AppError::Conflict("This sale is already finalized and can't be edited. Refund or void it instead.".into()));
            }
            conn.execute("DELETE FROM sale_usage WHERE sale_line_id IN (SELECT id FROM sale_lines WHERE sale_id = ?1)", [id])?;
            conn.execute("DELETE FROM sale_lines WHERE sale_id = ?1", [id])?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO sales (client_id, staff_id, location_id, appointment_id, sale_date, sale_discount, notes, prices_include_tax) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![d.client_id, d.staff_id, d.location_id, d.appointment_id, d.sale_date, d.sale_discount.to_string(), d.notes, include],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation => AppError::Conflict("This appointment already has a sale.".into()),
                e => e.into(),
            })?;
            conn.last_insert_rowid()
        }
    };
    for (i, (l, cat)) in d.lines.iter().zip(&cats).enumerate() {
        conn.execute(
            "INSERT INTO sale_lines (sale_id, kind, service_id, product_id, staff_id, description, variant_ids, qty, unit_price, line_discount, tax_category, planned_minutes, actual_minutes, sort)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                id,
                serde_json::to_value(l.kind)?.as_str().unwrap(),
                l.service_id,
                l.product_id,
                l.staff_id.or(d.staff_id),
                l.description.trim(),
                serde_json::to_string(&l.variant_ids)?,
                l.qty.to_string(),
                l.unit_price.to_string(),
                l.line_discount.to_string(),
                cat,
                l.planned_minutes,
                l.actual_minutes,
                i as i64
            ],
        )?;
        let lid = conn.last_insert_rowid();
        for u in &l.usage {
            conn.execute(
                "INSERT INTO sale_usage (sale_line_id, product_id, planned_qty, qty, unit) VALUES (?1,?2,?3,?4,?5)",
                params![lid, u.product_id, u.planned_qty.to_string(), u.qty.to_string(), u.unit],
            )?;
        }
    }
    Ok(id)
}

pub fn delete_draft(conn: &Connection, id: i64) -> AppResult<()> {
    let status: Option<String> = conn.query_row("SELECT status FROM sales WHERE id = ?1", [id], |r| r.get(0)).optional()?;
    match status.as_deref() {
        Some("draft") => {
            if conn.query_row("SELECT COUNT(*) FROM payments WHERE sale_id = ?1 AND voided_at IS NULL", [id], |r| r.get::<_, i64>(0))? > 0 {
                return Err(AppError::msg("Remove the payments recorded on this draft first."));
            }
            conn.execute("DELETE FROM payments WHERE sale_id = ?1", [id])?;
            conn.execute("DELETE FROM sale_usage WHERE sale_line_id IN (SELECT id FROM sale_lines WHERE sale_id = ?1)", [id])?;
            conn.execute("DELETE FROM sale_lines WHERE sale_id = ?1", [id])?;
            conn.execute("DELETE FROM sales WHERE id = ?1", [id])?;
            audit(conn, "sale", Some(id), "delete_draft", &format!("Draft sale {id} deleted"), None)?;
            Ok(())
        }
        Some(_) => Err(AppError::Conflict("Only drafts can be deleted. Void or refund a finalized sale.".into())),
        None => Err(AppError::NotFound("Sale not found.".into())),
    }
}

/// Start a draft from an appointment, with the recipe's planned usage filled in as actual usage
/// (edit it to what was really used). Returns the existing sale if one was already started.
pub fn draft_from_appointment(conn: &Connection, appointment_id: i64) -> AppResult<i64> {
    if let Some(id) = conn.query_row("SELECT id FROM sales WHERE appointment_id = ?1", [appointment_id], |r| r.get::<_, i64>(0)).optional()? {
        return Ok(id);
    }
    let a = super::appointments::get(conn, appointment_id)?;
    if a.status == "cancelled" || a.status == "no_show" {
        return Err(AppError::msg("This appointment was cancelled or marked as a no-show."));
    }
    let mut lines = Vec::new();
    for l in &a.lines {
        let s = services::get(conn, l.service_id)?;
        let (time, factor, price_delta, names) = services::apply_variants(&s.input, &l.variant_ids)?;
        let qty = Decimal::from(l.qty);
        let usage = material_lines(conn, &s.input.recipe, factor)?
            .into_iter()
            .map(|m| UsageInput { product_id: m.product_id, planned_qty: m.qty * qty, qty: m.qty * qty, unit: m.unit })
            .collect();
        lines.push(SaleLineInput {
            kind: LineKind::Service,
            service_id: Some(l.service_id),
            product_id: None,
            staff_id: Some(a.staff_id),
            description: if names.is_empty() { s.input.name.clone() } else { format!("{} ({})", s.input.name, names.join(", ")) },
            variant_ids: l.variant_ids.clone(),
            qty,
            unit_price: s.input.price.unwrap_or_default() + price_delta,
            line_discount: Decimal::ZERO,
            planned_minutes: Some(time.occupied_min() * l.qty),
            actual_minutes: None,
            usage,
        });
    }
    let id = save_draft(
        conn,
        &SaleDraft {
            id: None,
            client_id: a.client_id,
            staff_id: Some(a.staff_id),
            location_id: a.location_id,
            appointment_id: Some(appointment_id),
            sale_date: a.starts_at[..10].to_string(),
            sale_discount: Decimal::ZERO,
            notes: String::new(),
            lines,
        },
    )?;
    conn.execute("UPDATE appointments SET status = 'checked_in', updated_at = ?1 WHERE id = ?2 AND status = 'scheduled'", params![now_utc(), appointment_id])?;
    audit(conn, "sale", Some(id), "create", &format!("Checkout started for appointment {appointment_id}"), None)?;
    Ok(id)
}

// ---------------------------------------------------------------- read

fn load_draft(conn: &Connection, id: i64) -> AppResult<(SaleDraft, String)> {
    let (mut d, status) = conn
        .query_row(
            "SELECT client_id, staff_id, location_id, appointment_id, sale_date, sale_discount, notes, status FROM sales WHERE id = ?1",
            [id],
            |r| {
                Ok((
                    SaleDraft {
                        id: Some(id),
                        client_id: r.get(0)?,
                        staff_id: r.get(1)?,
                        location_id: r.get(2)?,
                        appointment_id: r.get(3)?,
                        sale_date: r.get(4)?,
                        sale_discount: dec(r, "sale_discount")?,
                        notes: r.get(6)?,
                        lines: vec![],
                    },
                    r.get::<_, String>(7)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("Sale {id} not found.")))?;
    let mut st = conn.prepare(
        "SELECT id, kind, service_id, product_id, staff_id, description, variant_ids, qty, unit_price, line_discount, planned_minutes, actual_minutes FROM sale_lines WHERE sale_id = ?1 ORDER BY sort, id",
    )?;
    let rows: Vec<(i64, SaleLineInput)> = st
        .query_map([id], |r| {
            let kind: String = r.get(1)?;
            Ok((
                r.get(0)?,
                SaleLineInput {
                    kind: serde_json::from_value(serde_json::Value::String(kind)).unwrap_or(LineKind::Service),
                    service_id: r.get(2)?,
                    product_id: r.get(3)?,
                    staff_id: r.get(4)?,
                    description: r.get(5)?,
                    variant_ids: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(),
                    qty: dec(r, "qty")?,
                    unit_price: dec(r, "unit_price")?,
                    line_discount: dec(r, "line_discount")?,
                    planned_minutes: r.get(10)?,
                    actual_minutes: r.get(11)?,
                    usage: vec![],
                },
            ))
        })?
        .collect::<Result<_, _>>()?;
    let mut ust = conn.prepare("SELECT product_id, planned_qty, qty, unit FROM sale_usage WHERE sale_line_id = ?1 ORDER BY id")?;
    for (lid, mut l) in rows {
        l.usage = ust
            .query_map([lid], |r| Ok(UsageInput { product_id: r.get(0)?, planned_qty: dec(r, "planned_qty")?, qty: dec(r, "qty")?, unit: r.get(3)? }))?
            .collect::<Result<_, _>>()?;
        d.lines.push(l);
    }
    Ok((d, status))
}

fn line_ids(conn: &Connection, sale_id: i64) -> AppResult<Vec<i64>> {
    let mut st = conn.prepare("SELECT id FROM sale_lines WHERE sale_id = ?1 ORDER BY sort, id")?;
    let rows = st.query_map([sale_id], |r| r.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn payments(conn: &Connection, sale_id: i64) -> AppResult<Vec<Payment>> {
    let mut st = conn.prepare("SELECT id, refund_id, method, amount, fee, reference, paid_on, voided_at FROM payments WHERE sale_id = ?1 ORDER BY id")?;
    let rows = st.query_map([sale_id], |r| {
        Ok(Payment {
            id: r.get(0)?,
            refund_id: r.get(1)?,
            method: r.get(2)?,
            amount: dec(r, "amount")?,
            fee: dec(r, "fee")?,
            reference: r.get(5)?,
            paid_on: r.get(6)?,
            voided: r.get::<_, Option<String>>(7)?.is_some(),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn get(conn: &Connection, id: i64) -> AppResult<SaleView> {
    let (d, status) = load_draft(conn, id)?;
    let head = conn.query_row(
        "SELECT s.number, s.created_at, s.finalized_at, s.voided_at, s.void_reason, c.first_name || ' ' || c.last_name, st.name, l.name,
                s.subtotal, s.discount_total, s.tax_total, s.tip_total, s.total, s.tax_label, s.tax_rate, s.tax_rate_status, s.prices_include_tax
         FROM sales s LEFT JOIN clients c ON c.id = s.client_id LEFT JOIN staff st ON st.id = s.staff_id LEFT JOIN locations l ON l.id = s.location_id WHERE s.id = ?1",
        [id],
        |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                [opt_dec(r, "subtotal")?, opt_dec(r, "discount_total")?, opt_dec(r, "tax_total")?, opt_dec(r, "tip_total")?, opt_dec(r, "total")?],
                r.get::<_, Option<String>>(13)?,
                opt_dec(r, "tax_rate")?,
                r.get::<_, Option<String>>(15)?,
                r.get::<_, i64>(16)? != 0,
            ))
        },
    )?;
    let ids = line_ids(conn, id)?;
    let mut lines = Vec::new();
    for lid in &ids {
        let snap = conn.query_row(
            "SELECT gross, sale_discount_share, net, tax, taxability, taxability_basis, tax_category, materials_cost, planned_materials_cost, labor_cost,
                    overhead_cost, other_direct_cost, commission, retail_cost,
                    (SELECT COALESCE(GROUP_CONCAT(rl.qty || '|' || rl.net || '|' || rl.tax, ';'), '') FROM refund_lines rl WHERE rl.sale_line_id = sale_lines.id)
             FROM sale_lines WHERE id = ?1",
            [lid],
            |r| {
                let refunds: String = r.get(14)?;
                let mut rq = Decimal::ZERO;
                let mut rn = Decimal::ZERO;
                let mut rt = Decimal::ZERO;
                for part in refunds.split(';').filter(|p| !p.is_empty()) {
                    let v: Vec<Decimal> = part.split('|').map(|x| x.parse().unwrap_or_default()).collect();
                    rq += v[0];
                    rn += v[1];
                    rt += v[2];
                }
                Ok(LineSnapshot {
                    id: *lid,
                    gross: opt_dec(r, "gross")?,
                    sale_discount_share: opt_dec(r, "sale_discount_share")?,
                    net: opt_dec(r, "net")?,
                    tax: opt_dec(r, "tax")?,
                    taxability: r.get(4)?,
                    taxability_basis: r.get(5)?,
                    tax_category: r.get(6)?,
                    materials_cost: opt_dec(r, "materials_cost")?,
                    planned_materials_cost: opt_dec(r, "planned_materials_cost")?,
                    labor_cost: opt_dec(r, "labor_cost")?,
                    overhead_cost: opt_dec(r, "overhead_cost")?,
                    other_direct_cost: opt_dec(r, "other_direct_cost")?,
                    commission: opt_dec(r, "commission")?,
                    retail_cost: opt_dec(r, "retail_cost")?,
                    refunded_qty: rq,
                    refunded_net: rn,
                    refunded_tax: rt,
                    usage_costs: vec![],
                })
            },
        )?;
        let mut snap = snap;
        let mut ust = conn.prepare("SELECT cost FROM sale_usage WHERE sale_line_id = ?1 ORDER BY id")?;
        snap.usage_costs = ust.query_map([lid], |r| opt_dec(r, "cost"))?.collect::<Result<_, _>>()?;
        lines.push(snap);
    }
    let pays = payments(conn, id)?;
    let mut rst = conn.prepare("SELECT id, number, refund_date, reason, net_total, tax_total, tip_total, total FROM refunds WHERE sale_id = ?1 ORDER BY id")?;
    let refunds: Vec<RefundView> = rst
        .query_map([id], |r| {
            Ok(RefundView {
                id: r.get(0)?,
                number: r.get(1)?,
                refund_date: r.get(2)?,
                reason: r.get(3)?,
                net_total: dec(r, "net_total")?,
                tax_total: dec(r, "tax_total")?,
                tip_total: dec(r, "tip_total")?,
                total: dec(r, "total")?,
            })
        })?
        .collect::<Result<_, _>>()?;

    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    let (totals, tax_label, tax_rate, tax_rate_status) = if status == "draft" {
        let cats: Vec<String> = d.lines.iter().map(|l| line_category(conn, l)).collect::<AppResult<_>>()?;
        let res = resolve_tax(conn, d.location_id, &cats)?;
        let out = compute(&line_ins(&d, &res), d.sale_discount, res.rate_pct, get_business(conn)?.prices_include_tax)?;
        blockers.extend(out.unresolved.iter().cloned());
        if d.lines.is_empty() {
            blockers.push("Add at least one line.".into());
        }
        if res.rate_status.as_deref() == Some("stale") {
            warnings.push("The location's tax rate is stale; refresh it on the Sales tax page before finalizing if rates may have changed.".into());
        }
        for (l, i) in d.lines.iter().zip(0..) {
            if l.kind == LineKind::Service && l.unit_price.is_zero() {
                warnings.push(format!("Line {} has no price.", i + 1));
            }
        }
        (out, res.label, res.rate_pct, res.rate_status)
    } else {
        let [sub, disc, tax_t, tip, total] = head.8;
        let out = SaleOut {
            lines: lines.iter().map(|l| crate::domain::sale::LineOut { gross: l.gross.unwrap_or_default(), sale_discount_share: l.sale_discount_share.unwrap_or_default(), net: l.net.unwrap_or_default(), tax: l.tax.unwrap_or_default() }).collect(),
            subtotal: sub.unwrap_or_default(),
            discount_total: disc.unwrap_or_default(),
            tax_total: tax_t.unwrap_or_default(),
            tip_total: tip.unwrap_or_default(),
            total: total.unwrap_or_default(),
            tax_resolved: true,
            unresolved: vec![],
        };
        (out, head.9.clone(), head.10, head.11.clone())
    };
    let paid: Decimal = pays.iter().filter(|p| !p.voided && p.refund_id.is_none()).map(|p| p.amount).sum();
    let refunded: Decimal = refunds.iter().map(|r| r.total).sum();
    let balance = if status == "voided" { Decimal::ZERO } else { totals.total - refunded - (paid - refunded_paid(&pays)) };
    Ok(SaleView {
        number: head.0,
        created_at: head.1,
        finalized_at: head.2,
        voided_at: head.3,
        void_reason: head.4,
        client_name: head.5.map(|n| n.trim().to_string()),
        staff_name: head.6,
        location_name: head.7,
        status,
        draft: d,
        totals,
        tax_label,
        tax_rate,
        tax_rate_status,
        lines,
        payments: pays,
        refunds,
        paid,
        refunded,
        balance,
        blockers,
        warnings,
    })
}

/// Money paid back to the client through refund payments (stored as negative amounts).
fn refunded_paid(p: &[Payment]) -> Decimal {
    -p.iter().filter(|p| !p.voided && p.refund_id.is_some()).map(|p| p.amount).sum::<Decimal>()
}

// ---------------------------------------------------------------- finalize

fn profile_for_line(conn: &Connection, staff_id: Option<i64>, service_profile: Option<i64>) -> AppResult<Option<super::profiles::ProfileView>> {
    let staff_profile = match staff_id {
        Some(sid) => staff_list(conn)?.into_iter().find(|s| s.id == Some(sid)).and_then(|s| s.default_profile_id),
        None => None,
    };
    match staff_profile.or(service_profile) {
        Some(pid) => Ok(Some(super::profiles::get(conn, pid)?)),
        None => Ok(None),
    }
}

/// Finalize a draft. Stock is deducted here, exactly once: the status guard and the ledger's unique
/// source index both reject a second attempt.
pub fn finalize(conn: &Connection, id: i64) -> AppResult<SaleView> {
    let view = get(conn, id)?;
    if view.status != "draft" {
        return Err(AppError::Conflict(format!("Sale {} is already {}.", view.number.unwrap_or(id.to_string()), view.status)));
    }
    if !view.blockers.is_empty() {
        return Err(AppError::msg(format!("This sale can't be finalized yet: {}", view.blockers.join(" "))));
    }
    let d = &view.draft;
    let cats: Vec<String> = d.lines.iter().map(|l| line_category(conn, l)).collect::<AppResult<_>>()?;
    let res = resolve_tax(conn, d.location_id, &cats)?;
    let include = get_business(conn)?.prices_include_tax;
    let out = compute(&line_ins(d, &res), d.sale_discount, res.rate_pct, include)?;

    let next: i64 = conn.query_row("SELECT COALESCE(MAX(CAST(substr(number, 3) AS INTEGER)), 0) + 1 FROM sales WHERE number IS NOT NULL", [], |r| r.get(0))?;
    let number = format!("S-{next:05}");
    let n = conn.execute(
        "UPDATE sales SET status='finalized', finalized_at=?1, number=?2, subtotal=?3, discount_total=?4, tax_total=?5, tip_total=?6, total=?7,
         tax_rate_set_id=?8, tax_rate=?9, tax_label=?10, tax_rate_status=?11, prices_include_tax=?12 WHERE id=?13 AND status='draft'",
        params![
            now_utc(),
            number,
            out.subtotal.to_string(),
            out.discount_total.to_string(),
            out.tax_total.to_string(),
            out.tip_total.to_string(),
            out.total.to_string(),
            res.rate_set_id,
            res.rate_pct.map(|r| r.to_string()),
            res.label,
            res.rate_status,
            include as i64,
            id
        ],
    )?;
    if n != 1 {
        return Err(AppError::Conflict("This sale was already finalized (duplicate submission ignored).".into()));
    }
    let ids = line_ids(conn, id)?;
    for (((lid, l), lo), (taxability, basis)) in ids.iter().zip(&d.lines).zip(&out.lines).zip(&res.taxability) {
        let ctx = |src: (&'static str, i64)| (d.sale_date.clone(), src);
        let mut materials = None;
        let mut planned_materials = None;
        let mut labor = None;
        let mut overhead = None;
        let mut other_direct = None;
        let mut commission = None;
        let mut retail_cost = None;
        let mut profile_version = None;
        let mut recipe_version = None;
        match l.kind {
            LineKind::Service => {
                let s = services::get(conn, l.service_id.unwrap())?;
                recipe_version = s.recipe_version_id;
                let mut mat = Decimal::ZERO;
                let mut ust = conn.prepare("SELECT id, product_id, qty, unit FROM sale_usage WHERE sale_line_id = ?1 ORDER BY id")?;
                let usage: Vec<(i64, i64, Decimal, String)> =
                    ust.query_map([lid], |r| Ok((r.get(0)?, r.get(1)?, dec(r, "qty")?, r.get(3)?)))?.collect::<Result<_, _>>()?;
                for (uid, pid, qty, unit) in usage {
                    let qb = product(conn, pid)?.units().to_base(qty, &unit)?;
                    if qb.is_zero() {
                        conn.execute("UPDATE sale_usage SET qty_base='0', cost='0' WHERE id=?1", [uid])?;
                        continue;
                    }
                    let (date, src) = ctx(("sale_usage", uid));
                    let posted = post(conn, pid, Op::Issue { kind: "service_use", qty: qb }, PostCtx { occurred_on: &date, storage_id: None, source: Some(src), note: &number })?;
                    let cost = -posted.value;
                    mat += cost;
                    conn.execute("UPDATE sale_usage SET qty_base=?1, cost=?2, ledger_id=?3 WHERE id=?4", params![qb.normalize().to_string(), cost.to_string(), posted.ledger_id, uid])?;
                }
                materials = Some(mat);
                let (time, factor, _, _) = services::apply_variants(&s.input, &l.variant_ids)?;
                let planned: Decimal = material_lines(conn, &s.input.recipe, factor)?.iter().map(|m| m.cost).sum::<Decimal>() * l.qty;
                planned_materials = Some(planned);
                other_direct = Some(s.input.other_direct_cost * l.qty);
                if let Some(prof) = profile_for_line(conn, l.staff_id.or(d.staff_id), s.input.profile_id)? {
                    if let Ok(rates) = prof.data.rates() {
                        // Recorded actual chair time (for the whole line) scales the planned split of
                        // working vs processing time; otherwise the plan is used.
                        let planned_occ = time.occupied_min();
                        let scale = match l.actual_minutes {
                            Some(a) if a > 0 && planned_occ > 0 => Decimal::from(a) / (Decimal::from(planned_occ) * l.qty),
                            _ => Decimal::ONE,
                        } * l.qty;
                        let work_h = hours(time.working_min()) * scale;
                        let ovh_h = match prof.data.overhead_basis {
                            crate::domain::profile::OverheadBasis::Occupied => hours(time.occupied_min()),
                            crate::domain::profile::OverheadBasis::HandsOn => hours(time.working_min()),
                        } * scale;
                        labor = Some(work_h * rates.labor_cost_per_billable_hour);
                        overhead = Some(ovh_h * rates.overhead_per_billable_hour);
                        commission = Some(lo.net * frac(rates.commission_pct));
                        profile_version = Some(prof.version_id);
                    }
                }
            }
            LineKind::Retail => {
                let p = product(conn, l.product_id.unwrap())?;
                let qb = p.units().to_base(l.qty, &p.stock_unit)?;
                let (date, src) = ctx(("sale_line", *lid));
                let posted = post(conn, p.id, Op::Issue { kind: "retail_sale", qty: qb }, PostCtx { occurred_on: &date, storage_id: p.default_storage_id, source: Some(src), note: &number })?;
                retail_cost = Some(-posted.value);
                if let Some(prof) = profile_for_line(conn, l.staff_id.or(d.staff_id), None)? {
                    commission = Some(lo.net * frac(prof.data.retail_commission_pct));
                    profile_version = Some(prof.version_id);
                }
            }
            LineKind::Tip => {}
        }
        let s = |d: Option<Decimal>| d.map(|d| d.to_string());
        conn.execute(
            "UPDATE sale_lines SET gross=?1, sale_discount_share=?2, net=?3, tax=?4, taxability=?5, taxability_basis=?6, profile_version_id=?7, recipe_version_id=?8,
             materials_cost=?9, planned_materials_cost=?10, labor_cost=?11, overhead_cost=?12, other_direct_cost=?13, commission=?14, retail_cost=?15 WHERE id=?16",
            params![
                lo.gross.to_string(),
                lo.sale_discount_share.to_string(),
                lo.net.to_string(),
                lo.tax.to_string(),
                serde_json::to_value(taxability)?.as_str().unwrap(),
                basis,
                profile_version,
                recipe_version,
                s(materials),
                s(planned_materials),
                s(labor),
                s(overhead),
                s(other_direct),
                s(commission),
                s(retail_cost),
                lid
            ],
        )?;
    }
    if let Some(a) = d.appointment_id {
        conn.execute("UPDATE appointments SET status='completed', updated_at=?1 WHERE id=?2", params![now_utc(), a])?;
    }
    audit(conn, "sale", Some(id), "finalize", &format!("Sale {number} finalized, total {}", out.total), None)?;
    get(conn, id)
}

// ---------------------------------------------------------------- payments

pub fn add_payment(conn: &Connection, sale_id: i64, p: &PaymentInput) -> AppResult<i64> {
    let v = get(conn, sale_id)?;
    if v.status == "voided" {
        return Err(AppError::msg("This sale is void."));
    }
    if !PAYMENT_METHODS.contains(&p.method.as_str()) {
        return Err(AppError::invalid("method", "Choose cash, card, check, transfer or other."));
    }
    if p.amount <= Decimal::ZERO || p.amount != p.amount.round_dp(2) {
        return Err(AppError::invalid("amount", "Enter an amount in dollars and cents."));
    }
    crate::db::inventory::valid_date("paid_on", &p.paid_on)?;
    if p.amount > v.balance {
        return Err(AppError::invalid("amount", format!("That's more than the balance due (${}).", v.balance)));
    }
    // Card fees use the processing settings of the sale's staff member's work profile.
    let fee = if p.method == "card" {
        match profile_for_line(conn, v.draft.staff_id, None)? {
            Some(prof) => round_money(p.amount * frac(prof.data.processing_pct) + prof.data.processing_fixed),
            None => Decimal::ZERO,
        }
    } else {
        Decimal::ZERO
    };
    conn.execute(
        "INSERT INTO payments (sale_id, method, amount, fee, reference, paid_on) VALUES (?1,?2,?3,?4,?5,?6)",
        params![sale_id, p.method, p.amount.to_string(), fee.to_string(), p.reference.trim(), p.paid_on],
    )?;
    let id = conn.last_insert_rowid();
    audit(conn, "payment", Some(id), "create", &format!("{} payment of ${} on sale {sale_id}", p.method, p.amount), None)?;
    Ok(id)
}

pub fn void_payment(conn: &Connection, payment_id: i64) -> AppResult<()> {
    let n = conn.execute("UPDATE payments SET voided_at = ?1 WHERE id = ?2 AND voided_at IS NULL AND refund_id IS NULL", params![now_utc(), payment_id])?;
    if n == 0 {
        return Err(AppError::msg("That payment can't be removed."));
    }
    audit(conn, "payment", Some(payment_id), "void", "Payment entry removed", None)?;
    Ok(())
}

// ---------------------------------------------------------------- void & refund

pub fn void(conn: &Connection, id: i64, reason: &str) -> AppResult<()> {
    if reason.trim().is_empty() {
        return Err(AppError::invalid("reason", "Say why this sale is being voided."));
    }
    let has_refunds: bool = conn.query_row("SELECT EXISTS (SELECT 1 FROM refunds WHERE sale_id = ?1)", [id], |r| r.get(0))?;
    if has_refunds {
        return Err(AppError::msg("This sale has refunds. Refund the remaining lines instead of voiding it."));
    }
    let n = conn.execute("UPDATE sales SET status='voided', voided_at=?1, void_reason=?2 WHERE id=?3 AND status='finalized'", params![now_utc(), reason.trim(), id])?;
    if n == 0 {
        return Err(AppError::Conflict("Only finalized sales can be voided (drafts can simply be deleted).".into()));
    }
    // A void means the sale was recorded by mistake: undo its stock movements too.
    let date = crate::db::tax::today().to_string();
    let mut st = conn.prepare(
        "SELECT l.id, l.product_id FROM stock_ledger l WHERE (l.source = 'sale_usage' AND l.source_id IN (SELECT u.id FROM sale_usage u JOIN sale_lines sl ON sl.id = u.sale_line_id WHERE sl.sale_id = ?1))
            OR (l.source = 'sale_line' AND l.source_id IN (SELECT id FROM sale_lines WHERE sale_id = ?1))",
    )?;
    let entries: Vec<(i64, i64)> = st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
    for (entry, pid) in entries {
        post(conn, pid, Op::Reverse { entry_id: entry }, PostCtx { occurred_on: &date, storage_id: None, source: None, note: &format!("Void: {}", reason.trim()) })?;
    }
    conn.execute("UPDATE payments SET voided_at = ?1 WHERE sale_id = ?2 AND voided_at IS NULL", params![now_utc(), id])?;
    if let Some(a) = conn.query_row("SELECT appointment_id FROM sales WHERE id = ?1", [id], |r| r.get::<_, Option<i64>>(0))? {
        conn.execute("UPDATE appointments SET status='checked_in' WHERE id = ?1", [a])?;
    }
    audit(conn, "sale", Some(id), "void", &format!("Sale voided: {}", reason.trim()), None)?;
    Ok(())
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RefundLineInput {
    pub sale_line_id: i64,
    pub qty: Decimal,
    /// Retail only: put the item back in stock at the cost it left with.
    pub restock: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct RefundInput {
    pub refund_date: String,
    pub reason: String,
    pub method: String,
    pub lines: Vec<RefundLineInput>,
}

pub fn refund(conn: &Connection, sale_id: i64, r: &RefundInput) -> AppResult<i64> {
    let v = get(conn, sale_id)?;
    if v.status != "finalized" {
        return Err(AppError::msg("Only finalized sales can be refunded."));
    }
    if r.reason.trim().is_empty() {
        return Err(AppError::invalid("reason", "Give a reason for the refund."));
    }
    if r.lines.is_empty() {
        return Err(AppError::invalid("lines", "Choose what to refund."));
    }
    if !PAYMENT_METHODS.contains(&r.method.as_str()) {
        return Err(AppError::invalid("method", "Choose how the money is returned."));
    }
    crate::db::inventory::valid_date("refund_date", &r.refund_date)?;
    // Validate every line before writing anything.
    let mut seen = std::collections::HashSet::new();
    for rl in &r.lines {
        let idx = v.lines.iter().position(|l| l.id == rl.sale_line_id).ok_or_else(|| AppError::msg("That line isn't on this sale."))?;
        if !seen.insert(rl.sale_line_id) {
            return Err(AppError::msg("Each line can appear only once in a refund."));
        }
        if rl.restock && v.draft.lines[idx].kind != LineKind::Retail {
            return Err(AppError::msg("Only retail items can go back into stock. Product used in a service isn't returned to stock by a refund."));
        }
        let snap = &v.lines[idx];
        refund_share(v.draft.lines[idx].qty, snap.net.unwrap_or_default(), snap.tax.unwrap_or_default(), snap.refunded_qty, snap.refunded_net, snap.refunded_tax, rl.qty)?;
    }
    let next: i64 = conn.query_row("SELECT COUNT(*) + 1 FROM refunds", [], |x| x.get(0))?;
    let number = format!("R-{next:05}");
    conn.execute(
        "INSERT INTO refunds (sale_id, number, refund_date, reason, net_total, tax_total, tip_total, total) VALUES (?1,?2,?3,?4,'0','0','0','0')",
        params![sale_id, number, r.refund_date, r.reason.trim()],
    )?;
    let rid = conn.last_insert_rowid();
    let (mut net_t, mut tax_t, mut tip_t) = (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
    for rl in &r.lines {
        let idx = v.lines.iter().position(|l| l.id == rl.sale_line_id).ok_or_else(|| AppError::msg("That line isn't on this sale."))?;
        let snap = &v.lines[idx];
        let line = &v.draft.lines[idx];
        let (net, tax) = refund_share(line.qty, snap.net.unwrap_or_default(), snap.tax.unwrap_or_default(), snap.refunded_qty, snap.refunded_net, snap.refunded_tax, rl.qty)?;
        conn.execute(
            "INSERT INTO refund_lines (refund_id, sale_line_id, qty, net, tax, restock) VALUES (?1,?2,?3,?4,?5,?6)",
            params![rid, rl.sale_line_id, rl.qty.to_string(), net.to_string(), tax.to_string(), rl.restock as i64],
        )?;
        let rlid = conn.last_insert_rowid();
        if rl.restock {
            let p = product(conn, line.product_id.unwrap())?;
            let qb = p.units().to_base(rl.qty, &p.stock_unit)?;
            let cost = snap.retail_cost.unwrap_or_default() * rl.qty / line.qty;
            let posted = post(
                conn,
                p.id,
                Op::Receive { kind: "customer_return", qty: qb, cost },
                PostCtx { occurred_on: &r.refund_date, storage_id: p.default_storage_id, source: Some(("refund_line", rlid)), note: &number },
            )?;
            conn.execute("UPDATE refund_lines SET restock_cost=?1, ledger_id=?2 WHERE id=?3", params![cost.to_string(), posted.ledger_id, rlid])?;
        }
        if line.kind == LineKind::Tip {
            tip_t += net;
        } else {
            net_t += net;
        }
        tax_t += tax;
    }
    let total = net_t + tax_t + tip_t;
    conn.execute(
        "UPDATE refunds SET net_total=?1, tax_total=?2, tip_total=?3, total=?4 WHERE id=?5",
        params![net_t.to_string(), tax_t.to_string(), tip_t.to_string(), total.to_string(), rid],
    )?;
    conn.execute(
        "INSERT INTO payments (sale_id, refund_id, method, amount, reference, paid_on) VALUES (?1,?2,?3,?4,?5,?6)",
        params![sale_id, rid, r.method, (-total).to_string(), number, r.refund_date],
    )?;
    audit(conn, "refund", Some(rid), "create", &format!("Refund {number} of ${total} on sale {}", v.number.unwrap_or_default()), Some(serde_json::to_value(r)?))?;
    Ok(rid)
}

// ---------------------------------------------------------------- lists

#[derive(Serialize, Clone, Debug)]
pub struct SaleSummary {
    pub id: i64,
    pub number: Option<String>,
    pub status: String,
    pub sale_date: String,
    pub client_name: Option<String>,
    pub staff_name: Option<String>,
    pub total: Option<Decimal>,
    pub lines: i64,
    pub refunded: bool,
}

#[derive(Deserialize, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct SaleFilter {
    pub from: Option<String>,
    pub to: Option<String>,
    pub status: Option<String>,
    pub client_id: Option<i64>,
}

pub fn list(conn: &Connection, f: &SaleFilter) -> AppResult<Vec<SaleSummary>> {
    let mut st = conn.prepare(
        "SELECT s.id, s.number, s.status, s.sale_date, TRIM(c.first_name || ' ' || c.last_name), st.name, s.total,
                (SELECT COUNT(*) FROM sale_lines WHERE sale_id = s.id), EXISTS (SELECT 1 FROM refunds WHERE sale_id = s.id)
         FROM sales s LEFT JOIN clients c ON c.id = s.client_id LEFT JOIN staff st ON st.id = s.staff_id
         WHERE (?1 IS NULL OR s.sale_date >= ?1) AND (?2 IS NULL OR s.sale_date <= ?2) AND (?3 IS NULL OR s.status = ?3) AND (?4 IS NULL OR s.client_id = ?4)
         ORDER BY s.sale_date DESC, s.id DESC LIMIT 2000",
    )?;
    let rows = st.query_map(params![f.from, f.to, f.status, f.client_id], |r| {
        Ok(SaleSummary {
            id: r.get(0)?,
            number: r.get(1)?,
            status: r.get(2)?,
            sale_date: r.get(3)?,
            client_name: r.get(4)?,
            staff_name: r.get(5)?,
            total: opt_dec(r, "total")?,
            lines: r.get(7)?,
            refunded: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::db::core::{save_business, save_location, save_staff, Business, Location, Staff};
    use crate::db::inventory::tests::{buy, product_input};
    use crate::db::inventory::{save_product, ProductInput};
    use crate::db::open_memory;
    use crate::db::tax::{save_rate, set_rule, RateSetInput};
    use crate::domain::profile::sample;
    use rust_decimal_macros::dec;

    pub struct Fixture {
        pub conn: Connection,
        pub loc: i64,
        pub staff: i64,
        pub profile: i64,
        pub dev: i64,
        pub shampoo: i64,
        pub service: i64,
    }

    pub fn fixture() -> Fixture {
        let conn = open_memory();
        let loc = save_location(
            &conn,
            &Location {
                id: None,
                name: "Main".into(),
                address_line: "1 Main".into(),
                city: "Tumwater".into(),
                state: "WA".into(),
                postal_code: "98501".into(),
                latitude: None,
                longitude: None,
                geo_precision: None,
                geo_source: None,
                state_fips: None,
                county_fips: None,
                place_fips: None,
                archived: false,
            },
        )
        .unwrap();
        let mut b = Business::default();
        b.name = "S".into();
        b.primary_location_id = Some(loc);
        save_business(&conn, &b).unwrap();
        let profile = crate::db::profiles::save(&conn, None, "Me", &sample()).unwrap();
        let staff = save_staff(&conn, &Staff { id: None, name: "Ana".into(), color: "grape".into(), default_profile_id: Some(profile), location_id: Some(loc), archived: false }).unwrap();
        let dev = save_product(&conn, &product_input("Developer", "fl_oz")).unwrap();
        buy(&conn, dev, dec!(32), "fl_oz", dec!(24));
        let shampoo = save_product(&conn, &ProductInput { category: "retail".into(), retail_price: Some(dec!(24)), ..product_input("Shampoo", "piece") }).unwrap();
        buy(&conn, shampoo, dec!(10), "piece", dec!(100));
        let mut s = crate::db::services::tests::service_input(profile, dev);
        s.variants.clear();
        let service = crate::db::services::save(&conn, &s).unwrap();
        Fixture { conn, loc, staff, profile, dev, shampoo, service }
    }

    pub fn resolve_tax_setup(f: &Fixture) {
        set_rule(&f.conn, f.loc, "service", "taxable", "test").unwrap();
        set_rule(&f.conn, f.loc, "retail", "taxable", "test").unwrap();
        set_rule(&f.conn, f.loc, "tips", "exempt", "test").unwrap();
        save_rate(
            &f.conn,
            &RateSetInput {
                location_id: f.loc,
                state_rate: None,
                county_rate: None,
                city_rate: None,
                district_rate: None,
                total_rate: Some(dec!(10)),
                jurisdiction_label: "Test".into(),
                jurisdiction_code: None,
                source: "manual".into(),
                source_url: None,
                precision: "address".into(),
                status: "manual".into(),
                effective_date: None,
                dataset_period: None,
                retrieved_at: None,
                note: String::new(),
            },
        )
        .unwrap();
    }

    pub fn draft(f: &Fixture) -> SaleDraft {
        SaleDraft {
            id: None,
            client_id: None,
            staff_id: Some(f.staff),
            location_id: Some(f.loc),
            appointment_id: None,
            sale_date: "2026-10-05".into(),
            sale_discount: dec!(0),
            notes: String::new(),
            lines: vec![
                SaleLineInput {
                    kind: LineKind::Service,
                    service_id: Some(f.service),
                    product_id: None,
                    staff_id: None,
                    description: "Root touch-up".into(),
                    variant_ids: vec![],
                    qty: dec!(1),
                    unit_price: dec!(95),
                    line_discount: dec!(0),
                    planned_minutes: Some(95),
                    actual_minutes: None,
                    usage: vec![UsageInput { product_id: f.dev, planned_qty: dec!(2), qty: dec!(2.5), unit: "fl_oz".into() }],
                },
                SaleLineInput {
                    kind: LineKind::Retail,
                    service_id: None,
                    product_id: Some(f.shampoo),
                    staff_id: None,
                    description: "Shampoo".into(),
                    variant_ids: vec![],
                    qty: dec!(2),
                    unit_price: dec!(24),
                    line_discount: dec!(0),
                    planned_minutes: None,
                    actual_minutes: None,
                    usage: vec![],
                },
                SaleLineInput {
                    kind: LineKind::Tip,
                    service_id: None,
                    product_id: None,
                    staff_id: None,
                    description: "Tip".into(),
                    variant_ids: vec![],
                    qty: dec!(1),
                    unit_price: dec!(20),
                    line_discount: dec!(0),
                    planned_minutes: None,
                    actual_minutes: None,
                    usage: vec![],
                },
            ],
        }
    }

    #[test]
    fn unresolved_tax_blocks_finalize_and_drafts_move_no_stock() {
        let f = fixture();
        let id = save_draft(&f.conn, &draft(&f)).unwrap();
        let v = get(&f.conn, id).unwrap();
        assert!(!v.blockers.is_empty());
        assert!(finalize(&f.conn, id).is_err());
        // editing a draft many times never touches stock
        save_draft(&f.conn, &SaleDraft { id: Some(id), ..draft(&f) }).unwrap();
        assert_eq!(product(&f.conn, f.dev).unwrap().on_hand, dec!(32));
    }

    #[test]
    fn finalize_deducts_actual_usage_once_and_snapshots() {
        let f = fixture();
        resolve_tax_setup(&f);
        let id = save_draft(&f.conn, &draft(&f)).unwrap();
        let v = finalize(&f.conn, id).unwrap();
        assert_eq!(v.status, "finalized");
        assert_eq!(v.number.as_deref(), Some("S-00001"));
        // 95 + 48 taxable at 10% = 14.30; tip exempt
        assert_eq!(v.totals.tax_total, dec!(14.3));
        assert_eq!(v.totals.total, dec!(95) + dec!(48) + dec!(14.3) + dec!(20));
        // actual 2.5 fl oz used (planned 2) at $0.75 = $1.875
        assert_eq!(v.lines[0].materials_cost, Some(dec!(1.875)));
        assert_eq!(v.lines[0].planned_materials_cost, Some(dec!(1.5)));
        assert_eq!(product(&f.conn, f.dev).unwrap().on_hand, dec!(29.5));
        assert_eq!(product(&f.conn, f.shampoo).unwrap().on_hand, dec!(8));
        assert_eq!(v.lines[1].retail_cost, Some(dec!(20)));
        // second finalize is rejected and deducts nothing
        assert!(matches!(finalize(&f.conn, id), Err(AppError::Conflict(_))));
        assert_eq!(product(&f.conn, f.dev).unwrap().on_hand, dec!(29.5));
        // finalized sales can't be edited
        assert!(save_draft(&f.conn, &SaleDraft { id: Some(id), ..draft(&f) }).is_err());
    }

    #[test]
    fn history_survives_profile_and_tax_changes() {
        let f = fixture();
        resolve_tax_setup(&f);
        let id = save_draft(&f.conn, &draft(&f)).unwrap();
        let before = finalize(&f.conn, id).unwrap();
        let mut p = sample();
        p.hourly_rate = dec!(80);
        crate::db::profiles::save(&f.conn, Some(f.profile), "Me", &p).unwrap();
        set_rule(&f.conn, f.loc, "service", "exempt", "changed my mind").unwrap();
        crate::db::tax::clear_rate(&f.conn, f.loc, "x").unwrap();
        let after = get(&f.conn, id).unwrap();
        assert_eq!(after.totals.tax_total, before.totals.tax_total);
        assert_eq!(after.lines[0].labor_cost, before.lines[0].labor_cost);
        assert_eq!(after.lines[0].taxability.as_deref(), Some("taxable"));
        assert_eq!(after.tax_rate, Some(dec!(10)));
    }

    #[test]
    fn refunds_and_restock_rules() {
        let f = fixture();
        resolve_tax_setup(&f);
        let id = save_draft(&f.conn, &draft(&f)).unwrap();
        let v = finalize(&f.conn, id).unwrap();
        add_payment(&f.conn, id, &PaymentInput { method: "card".into(), amount: v.totals.total, reference: String::new(), paid_on: "2026-10-05".into() }).unwrap();
        assert!(add_payment(&f.conn, id, &PaymentInput { method: "cash".into(), amount: dec!(1), reference: String::new(), paid_on: "2026-10-05".into() }).is_err(), "overpayment");
        // service product can't be restocked by a refund
        let bad = refund(&f.conn, id, &RefundInput { refund_date: "2026-10-06".into(), reason: "x".into(), method: "card".into(), lines: vec![RefundLineInput { sale_line_id: v.lines[0].id, qty: dec!(1), restock: true }] });
        assert!(bad.is_err());
        // refund one shampoo back to stock + the service without restock
        refund(
            &f.conn,
            id,
            &RefundInput {
                refund_date: "2026-10-06".into(),
                reason: "Unhappy".into(),
                method: "card".into(),
                lines: vec![RefundLineInput { sale_line_id: v.lines[1].id, qty: dec!(1), restock: true }, RefundLineInput { sale_line_id: v.lines[0].id, qty: dec!(1), restock: false }],
            },
        )
        .unwrap();
        let after = get(&f.conn, id).unwrap();
        assert_eq!(after.refunds[0].net_total, dec!(24) + dec!(95));
        assert_eq!(after.refunds[0].tax_total, dec!(2.4) + dec!(9.5));
        assert_eq!(product(&f.conn, f.shampoo).unwrap().on_hand, dec!(9)); // one back
        assert_eq!(product(&f.conn, f.dev).unwrap().on_hand, dec!(29.5)); // used product stays used
        assert_eq!(after.balance, dec!(0));
        // can't refund more than sold
        assert!(refund(&f.conn, id, &RefundInput { refund_date: "2026-10-06".into(), reason: "x".into(), method: "card".into(), lines: vec![RefundLineInput { sale_line_id: v.lines[0].id, qty: dec!(1), restock: false }] }).is_err());
        // with refunds, void is refused
        assert!(void(&f.conn, id, "oops").is_err());
    }

    #[test]
    fn void_reverses_stock() {
        let f = fixture();
        resolve_tax_setup(&f);
        let id = save_draft(&f.conn, &draft(&f)).unwrap();
        finalize(&f.conn, id).unwrap();
        void(&f.conn, id, "Entered twice").unwrap();
        assert_eq!(product(&f.conn, f.dev).unwrap().on_hand, dec!(32));
        assert_eq!(product(&f.conn, f.shampoo).unwrap().on_hand, dec!(10));
        assert_eq!(product(&f.conn, f.dev).unwrap().value, dec!(24));
        assert!(void(&f.conn, id, "again").is_err());
    }
}
