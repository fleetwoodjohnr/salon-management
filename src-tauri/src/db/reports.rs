//! Reports and dashboard metrics. Every figure is summed in Rust from the same fact rows used for
//! drill-down, so totals, groupings and charts reconcile with the underlying records.
//!
//! Definitions (also in docs/calculations.md):
//! - Sales are counted on their sale date; refunds on their refund date; fees with their sale.
//! - Net revenue = service + retail sales (after discounts, before tax) − refunds. Tips and sales
//!   tax are pass-through amounts reported separately, never revenue.
//! - Direct costs = materials used (ledger cost) + retail cost of goods (less restocked returns)
//!   + commissions + card processing fees.
//! - Gross profit = net revenue − direct costs.
//! - Service contribution (allocated) = gross profit − allocated labor − allocated overhead −
//!   other direct costs. Uses the work-profile estimates frozen on each sale.
//! - Operating result (actual) = gross profit − recorded expenses. It does NOT subtract allocated
//!   overhead or labor, so overhead is never counted twice.
//! - Inventory purchases are cash spent on stock; materials used is stock consumed. They differ.

use super::{dec, opt_dec};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Deserialize, Serialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReportFilter {
    pub from: String,
    pub to: String,
    pub location_id: Option<i64>,
    pub profile_id: Option<i64>,
    pub staff_id: Option<i64>,
    pub service_id: Option<i64>,
    pub category: Option<String>,
    pub product_id: Option<i64>,
    /// "all" (default) | "with_refunds" | "without_refunds"
    pub status: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct LineFact {
    pub sale_id: i64,
    pub number: String,
    pub sale_date: String,
    pub kind: String,
    pub description: String,
    pub service_id: Option<i64>,
    pub category: String,
    pub product_id: Option<i64>,
    pub staff_id: Option<i64>,
    pub staff_name: Option<String>,
    pub profile_id: Option<i64>,
    pub profile_name: Option<String>,
    pub qty: Decimal,
    pub net: Decimal,
    pub discount: Decimal,
    pub tax: Decimal,
    pub materials: Decimal,
    pub planned_materials: Decimal,
    pub retail_cost: Decimal,
    pub labor: Decimal,
    pub overhead: Decimal,
    pub other_direct: Decimal,
    pub commission: Decimal,
    pub fees: Decimal,
    pub planned_minutes: Option<i64>,
    pub actual_minutes: Option<i64>,
    pub taxability: String,
    pub tax_label: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct RefundFact {
    pub refund_id: i64,
    pub number: String,
    pub sale_id: i64,
    pub refund_date: String,
    pub kind: String,
    pub description: String,
    pub service_id: Option<i64>,
    pub category: String,
    pub product_id: Option<i64>,
    pub staff_id: Option<i64>,
    pub profile_id: Option<i64>,
    pub net: Decimal,
    pub tax: Decimal,
    pub restock_cost: Decimal,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub service_sales: Decimal,
    pub retail_sales: Decimal,
    pub discounts: Decimal,
    pub refunds: Decimal,
    pub net_revenue: Decimal,
    pub tips: Decimal,
    pub tax_collected: Decimal,
    pub materials_used: Decimal,
    pub retail_cogs: Decimal,
    pub commissions: Decimal,
    pub processing_fees: Decimal,
    pub direct_costs: Decimal,
    pub gross_profit: Decimal,
    pub gross_margin_pct: Option<Decimal>,
    pub labor_allocated: Decimal,
    pub overhead_allocated: Decimal,
    pub other_direct: Decimal,
    pub service_contribution: Decimal,
    pub expenses: Decimal,
    pub operating_result: Decimal,
    pub inventory_purchases: Decimal,
    pub stock_consumed: Decimal,
    pub waste_and_adjustments: Decimal,
    pub sales_count: i64,
    pub services_count: Decimal,
    pub average_ticket: Option<Decimal>,
    pub planned_materials: Decimal,
    pub planned_minutes: i64,
    pub actual_minutes: i64,
    pub lines_with_actual_time: i64,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct GroupRow {
    pub key: String,
    pub label: String,
    pub qty: Decimal,
    pub revenue: Decimal,
    pub refunds: Decimal,
    pub materials: Decimal,
    pub retail_cogs: Decimal,
    pub commission: Decimal,
    pub fees: Decimal,
    pub gross_profit: Decimal,
    pub labor: Decimal,
    pub overhead: Decimal,
    pub other_direct: Decimal,
    /// labor + overhead (the allocated estimates)
    pub allocated: Decimal,
    pub contribution: Decimal,
    pub margin_pct: Option<Decimal>,
    pub planned_materials: Decimal,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct PeriodPoint {
    pub period: String,
    pub net_revenue: Decimal,
    pub gross_profit: Decimal,
    pub contribution: Decimal,
    pub expenses: Decimal,
    pub operating_result: Decimal,
}

#[derive(Serialize, Clone, Debug)]
pub struct Report {
    pub filter: ReportFilter,
    pub summary: Summary,
    pub prior: Option<Summary>,
    pub prior_from: Option<String>,
    pub prior_to: Option<String>,
    pub groups: BTreeMap<String, Vec<GroupRow>>,
    pub trend: Vec<PeriodPoint>,
    pub notes: Vec<String>,
}

fn d0() -> Decimal {
    Decimal::ZERO
}

fn validate(f: &ReportFilter) -> AppResult<()> {
    crate::db::inventory::valid_date("from", &f.from)?;
    crate::db::inventory::valid_date("to", &f.to)?;
    if f.to < f.from {
        return Err(AppError::invalid("to", "The end date is before the start date."));
    }
    Ok(())
}

fn line_matches(f: &ReportFilter, service_id: Option<i64>, category: &str, product_id: Option<i64>, staff_id: Option<i64>, profile_id: Option<i64>) -> bool {
    f.service_id.is_none_or(|s| service_id == Some(s))
        && f.category.as_ref().is_none_or(|c| c.is_empty() || c == category)
        && f.product_id.is_none_or(|p| product_id == Some(p))
        && f.staff_id.is_none_or(|s| staff_id == Some(s))
        && f.profile_id.is_none_or(|p| profile_id == Some(p))
}

pub fn line_facts(conn: &Connection, f: &ReportFilter) -> AppResult<Vec<LineFact>> {
    validate(f)?;
    // Fees are allocated to each sale's non-tip lines in proportion to gross.
    let mut st = conn.prepare(
        "SELECT s.id, s.number, s.sale_date, l.kind, l.description, l.service_id, COALESCE(sv.category, p.subcategory, ''), l.product_id, l.staff_id, st.name,
                pv.profile_id, wp.name, l.qty, l.net, l.gross, l.line_discount, l.sale_discount_share, l.tax, l.materials_cost, l.planned_materials_cost,
                l.retail_cost, l.labor_cost, l.overhead_cost, l.other_direct_cost, l.commission, l.planned_minutes, l.actual_minutes,
                (SELECT COALESCE(GROUP_CONCAT(fee, ';'), '') FROM payments WHERE sale_id = s.id AND voided_at IS NULL AND refund_id IS NULL),
                (SELECT COALESCE(GROUP_CONCAT(gross, ';'), '') FROM sale_lines WHERE sale_id = s.id AND kind <> 'tip'),
                EXISTS (SELECT 1 FROM refunds WHERE sale_id = s.id), COALESCE(l.taxability, ''), COALESCE(s.tax_label, '')
         FROM sale_lines l JOIN sales s ON s.id = l.sale_id
         LEFT JOIN services sv ON sv.id = l.service_id LEFT JOIN products p ON p.id = l.product_id LEFT JOIN staff st ON st.id = l.staff_id
         LEFT JOIN work_profile_versions pv ON pv.id = l.profile_version_id LEFT JOIN work_profiles wp ON wp.id = pv.profile_id
         WHERE s.status = 'finalized' AND s.sale_date >= ?1 AND s.sale_date <= ?2 AND (?3 IS NULL OR s.location_id = ?3)
         ORDER BY s.sale_date, s.id, l.sort",
    )?;
    let mut rows = st.query(params![f.from, f.to, f.location_id])?;
    let mut out = Vec::new();
    let sum = |s: String| -> Decimal { s.split(';').filter(|x| !x.is_empty()).filter_map(|x| x.parse::<Decimal>().ok()).sum() };
    while let Some(r) = rows.next()? {
        let has_refund: bool = r.get(29)?;
        match f.status.as_deref() {
            Some("with_refunds") if !has_refund => continue,
            Some("without_refunds") if has_refund => continue,
            _ => {}
        }
        let kind: String = r.get(3)?;
        let service_id: Option<i64> = r.get(5)?;
        let category: String = r.get(6)?;
        let product_id: Option<i64> = r.get(7)?;
        let staff_id: Option<i64> = r.get(8)?;
        let profile_id: Option<i64> = r.get(10)?;
        if !line_matches(f, service_id, &category, product_id, staff_id, profile_id) {
            continue;
        }
        let gross = opt_dec(r, "gross")?.unwrap_or_default();
        let fees_total = sum(r.get(27)?);
        let gross_total = sum(r.get(28)?);
        let fees = if kind == "tip" || gross_total.is_zero() { d0() } else { fees_total * gross / gross_total };
        let o = |name: &str| opt_dec(r, name).map(|v| v.unwrap_or_default());
        out.push(LineFact {
            sale_id: r.get(0)?,
            number: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            sale_date: r.get(2)?,
            kind,
            description: r.get(4)?,
            service_id,
            category,
            product_id,
            staff_id,
            staff_name: r.get(9)?,
            profile_id,
            profile_name: r.get(11)?,
            qty: dec(r, "qty")?,
            net: o("net")?,
            discount: dec(r, "line_discount")? + o("sale_discount_share")?,
            tax: o("tax")?,
            materials: o("materials_cost")?,
            planned_materials: o("planned_materials_cost")?,
            retail_cost: o("retail_cost")?,
            labor: o("labor_cost")?,
            overhead: o("overhead_cost")?,
            other_direct: o("other_direct_cost")?,
            commission: o("commission")?,
            fees,
            planned_minutes: r.get(25)?,
            actual_minutes: r.get(26)?,
            taxability: r.get(30)?,
            tax_label: r.get(31)?,
        });
    }
    Ok(out)
}

pub fn refund_facts(conn: &Connection, f: &ReportFilter) -> AppResult<Vec<RefundFact>> {
    validate(f)?;
    let mut st = conn.prepare(
        "SELECT rf.id, rf.number, rf.sale_id, rf.refund_date, l.kind, l.description, l.service_id, COALESCE(sv.category, p.subcategory, ''), l.product_id, l.staff_id,
                pv.profile_id, rl.net, rl.tax, rl.restock_cost
         FROM refund_lines rl JOIN refunds rf ON rf.id = rl.refund_id JOIN sale_lines l ON l.id = rl.sale_line_id JOIN sales s ON s.id = rf.sale_id
         LEFT JOIN services sv ON sv.id = l.service_id LEFT JOIN products p ON p.id = l.product_id LEFT JOIN work_profile_versions pv ON pv.id = l.profile_version_id
         WHERE s.status = 'finalized' AND rf.refund_date >= ?1 AND rf.refund_date <= ?2 AND (?3 IS NULL OR s.location_id = ?3) ORDER BY rf.refund_date, rf.id",
    )?;
    let mut rows = st.query(params![f.from, f.to, f.location_id])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        let service_id: Option<i64> = r.get(6)?;
        let category: String = r.get(7)?;
        let product_id: Option<i64> = r.get(8)?;
        let staff_id: Option<i64> = r.get(9)?;
        let profile_id: Option<i64> = r.get(10)?;
        if !line_matches(f, service_id, &category, product_id, staff_id, profile_id) {
            continue;
        }
        out.push(RefundFact {
            refund_id: r.get(0)?,
            number: r.get(1)?,
            sale_id: r.get(2)?,
            refund_date: r.get(3)?,
            kind: r.get(4)?,
            description: r.get(5)?,
            service_id,
            category,
            product_id,
            staff_id,
            profile_id,
            net: dec(r, "net")?,
            tax: dec(r, "tax")?,
            restock_cost: opt_dec(r, "restock_cost")?.unwrap_or_default(),
        });
    }
    Ok(out)
}

fn sum_col(conn: &Connection, sql: &str, p: impl rusqlite::Params) -> AppResult<Decimal> {
    let mut st = conn.prepare(sql)?;
    let vals: Vec<String> = st.query_map(p, |r| r.get(0))?.collect::<Result<_, _>>()?;
    Ok(vals.iter().filter_map(|v| v.parse::<Decimal>().ok()).sum())
}

/// Line filters (service, staff, profile, product, category) don't apply to these business-wide
/// figures, so they are only included when no line filter is active.
fn line_filtered(f: &ReportFilter) -> bool {
    f.service_id.is_some() || f.staff_id.is_some() || f.profile_id.is_some() || f.product_id.is_some() || f.category.as_ref().is_some_and(|c| !c.is_empty())
}

pub fn summarize(conn: &Connection, f: &ReportFilter) -> AppResult<(Summary, Vec<LineFact>, Vec<RefundFact>)> {
    let lines = line_facts(conn, f)?;
    let refunds = refund_facts(conn, f)?;
    let mut s = Summary::default();
    let mut sales = std::collections::HashSet::new();
    for l in &lines {
        sales.insert(l.sale_id);
        match l.kind.as_str() {
            "service" => {
                s.service_sales += l.net;
                s.services_count += l.qty;
                s.materials_used += l.materials;
                s.planned_materials += l.planned_materials;
                s.labor_allocated += l.labor;
                s.overhead_allocated += l.overhead;
                s.other_direct += l.other_direct;
                if let (Some(p), Some(a)) = (l.planned_minutes, l.actual_minutes) {
                    s.planned_minutes += p;
                    s.actual_minutes += a;
                    s.lines_with_actual_time += 1;
                }
            }
            "retail" => {
                s.retail_sales += l.net;
                s.retail_cogs += l.retail_cost;
            }
            _ => s.tips += l.net,
        }
        if l.kind != "tip" {
            s.discounts += l.discount;
        }
        s.tax_collected += l.tax;
        s.commissions += l.commission;
        s.processing_fees += l.fees;
    }
    for r in &refunds {
        if r.kind == "tip" {
            s.tips -= r.net;
        } else {
            s.refunds += r.net;
        }
        s.tax_collected -= r.tax;
        s.retail_cogs -= r.restock_cost;
    }
    s.sales_count = sales.len() as i64;
    s.net_revenue = s.service_sales + s.retail_sales - s.refunds;
    s.direct_costs = s.materials_used + s.retail_cogs + s.commissions + s.processing_fees;
    s.gross_profit = s.net_revenue - s.direct_costs;
    s.gross_margin_pct = (!s.net_revenue.is_zero()).then(|| s.gross_profit / s.net_revenue * Decimal::ONE_HUNDRED);
    s.service_contribution = s.gross_profit - s.labor_allocated - s.overhead_allocated - s.other_direct;
    s.average_ticket = (s.sales_count > 0).then(|| (s.service_sales + s.retail_sales) / Decimal::from(s.sales_count));
    if !line_filtered(f) {
        s.expenses = sum_col(
            conn,
            "SELECT amount FROM expenses WHERE voided_at IS NULL AND expense_date >= ?1 AND expense_date <= ?2 AND (?3 IS NULL OR location_id = ?3 OR location_id IS NULL)",
            params![f.from, f.to, f.location_id],
        )?;
        s.inventory_purchases = sum_col(
            conn,
            "SELECT total FROM purchases WHERE kind = 'purchase' AND reversed_at IS NULL AND purchase_date >= ?1 AND purchase_date <= ?2",
            params![f.from, f.to],
        )?;
        s.waste_and_adjustments = -sum_col(
            conn,
            "SELECT value FROM stock_ledger WHERE kind IN ('waste','adjustment','revaluation','supplier_return') AND occurred_on >= ?1 AND occurred_on <= ?2",
            params![f.from, f.to],
        )?;
        s.stock_consumed = -sum_col(
            conn,
            "SELECT l.value FROM stock_ledger l WHERE l.occurred_on >= ?1 AND l.occurred_on <= ?2 AND (l.kind IN ('service_use','retail_sale','customer_return')
               OR (l.kind = 'reversal' AND (SELECT k.kind FROM stock_ledger k WHERE k.id = l.reversal_of) IN ('service_use','retail_sale','customer_return')))",
            params![f.from, f.to],
        )?;
    }
    s.operating_result = s.gross_profit - s.expenses;
    Ok((s, lines, refunds))
}

fn add_line(g: &mut GroupRow, l: &LineFact) {
    g.qty += l.qty;
    g.revenue += l.net;
    g.materials += l.materials;
    g.planned_materials += l.planned_materials;
    g.retail_cogs += l.retail_cost;
    g.commission += l.commission;
    g.fees += l.fees;
    g.labor += l.labor;
    g.overhead += l.overhead;
    g.other_direct += l.other_direct;
}

fn finish(mut rows: Vec<GroupRow>) -> Vec<GroupRow> {
    for g in rows.iter_mut() {
        let net = g.revenue - g.refunds;
        g.gross_profit = net - g.materials - g.retail_cogs - g.commission - g.fees;
        g.allocated = g.labor + g.overhead;
        g.contribution = g.gross_profit - g.labor - g.overhead - g.other_direct;
        g.margin_pct = (!net.is_zero()).then(|| g.contribution / net * Decimal::ONE_HUNDRED);
    }
    rows.sort_by(|a, b| b.revenue.cmp(&a.revenue));
    rows
}

pub fn group(lines: &[LineFact], refunds: &[RefundFact], by: &str) -> AppResult<Vec<GroupRow>> {
    let key_of = |kind: &str, service: Option<i64>, product: Option<i64>, staff: Option<i64>, profile: Option<i64>, category: &str| -> Option<String> {
        if kind == "tip" {
            return None;
        }
        Some(match by {
            "service" => match kind {
                "service" => format!("s{}", service.unwrap_or_default()),
                _ => "retail".into(),
            },
            "product" => match kind {
                "retail" => format!("p{}", product.unwrap_or_default()),
                _ => return None,
            },
            "staff" => staff.map(|s| s.to_string()).unwrap_or_else(|| "none".into()),
            "profile" => profile.map(|p| p.to_string()).unwrap_or_else(|| "none".into()),
            "category" => if category.is_empty() { "Uncategorized".into() } else { category.to_string() },
            _ => return None,
        })
    };
    if !matches!(by, "service" | "product" | "staff" | "profile" | "category") {
        return Err(AppError::invalid("group_by", "Unknown grouping."));
    }
    let mut map: BTreeMap<String, GroupRow> = BTreeMap::new();
    for l in lines {
        let Some(k) = key_of(&l.kind, l.service_id, l.product_id, l.staff_id, l.profile_id, &l.category) else { continue };
        let label = match by {
            "service" if l.kind == "service" => l.description.split(" (").next().unwrap_or(&l.description).to_string(),
            "service" => "Retail products".into(),
            "product" => l.description.clone(),
            "staff" => l.staff_name.clone().unwrap_or("Unassigned".into()),
            "profile" => l.profile_name.clone().unwrap_or("No profile".into()),
            _ => k.clone(),
        };
        let g = map.entry(k.clone()).or_insert_with(|| GroupRow { key: k, label, ..Default::default() });
        add_line(g, l);
    }
    for r in refunds {
        let Some(k) = key_of(&r.kind, r.service_id, r.product_id, r.staff_id, r.profile_id, &r.category) else { continue };
        let g = map.entry(k.clone()).or_insert_with(|| GroupRow { key: k, label: r.description.clone(), ..Default::default() });
        g.refunds += r.net;
        g.retail_cogs -= r.restock_cost;
    }
    Ok(finish(map.into_values().collect()))
}

fn bucket(date: &str, by: &str) -> String {
    match by {
        "month" => date[..7].to_string(),
        "week" => {
            let d = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
            use chrono::Datelike;
            (d - chrono::Duration::days(d.weekday().num_days_from_monday() as i64)).to_string()
        }
        _ => date.to_string(),
    }
}

pub fn trend(conn: &Connection, f: &ReportFilter, lines: &[LineFact], refunds: &[RefundFact], by: &str) -> AppResult<Vec<PeriodPoint>> {
    let mut map: BTreeMap<String, PeriodPoint> = BTreeMap::new();
    for l in lines.iter().filter(|l| l.kind != "tip") {
        let p = map.entry(bucket(&l.sale_date, by)).or_default();
        p.net_revenue += l.net;
        let gp = l.net - l.materials - l.retail_cost - l.commission - l.fees;
        p.gross_profit += gp;
        p.contribution += gp - l.labor - l.overhead - l.other_direct;
    }
    for r in refunds.iter().filter(|r| r.kind != "tip") {
        let p = map.entry(bucket(&r.refund_date, by)).or_default();
        p.net_revenue -= r.net;
        p.gross_profit -= r.net - r.restock_cost;
        p.contribution -= r.net - r.restock_cost;
    }
    if !line_filtered(f) {
        let mut st = conn.prepare("SELECT expense_date, amount FROM expenses WHERE voided_at IS NULL AND expense_date >= ?1 AND expense_date <= ?2 AND (?3 IS NULL OR location_id = ?3 OR location_id IS NULL)")?;
        let rows: Vec<(String, String)> = st.query_map(params![f.from, f.to, f.location_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        for (d, a) in rows {
            map.entry(bucket(&d, by)).or_default().expenses += a.parse::<Decimal>().unwrap_or_default();
        }
    }
    Ok(map
        .into_iter()
        .map(|(k, mut p)| {
            p.period = k;
            p.operating_result = p.gross_profit - p.expenses;
            p
        })
        .collect())
}

pub fn prior_range(from: &str, to: &str) -> (String, String) {
    let f = chrono::NaiveDate::parse_from_str(from, "%Y-%m-%d").unwrap();
    let t = chrono::NaiveDate::parse_from_str(to, "%Y-%m-%d").unwrap();
    let len = (t - f).num_days() + 1;
    let pt = f - chrono::Duration::days(1);
    let pf = pt - chrono::Duration::days(len - 1);
    (pf.to_string(), pt.to_string())
}

pub fn report(conn: &Connection, f: &ReportFilter, groups: &[String], trend_by: Option<&str>, compare: bool) -> AppResult<Report> {
    let (summary, lines, refunds) = summarize(conn, f)?;
    let mut g = BTreeMap::new();
    for by in groups {
        g.insert(by.clone(), group(&lines, &refunds, by)?);
    }
    let trend_pts = match trend_by {
        Some(b) => trend(conn, f, &lines, &refunds, b)?,
        None => vec![],
    };
    let (prior, prior_from, prior_to) = if compare {
        let (pf, pt) = prior_range(&f.from, &f.to);
        let pfilter = ReportFilter { from: pf.clone(), to: pt.clone(), ..f.clone() };
        (Some(summarize(conn, &pfilter)?.0), Some(pf), Some(pt))
    } else {
        (None, None, None)
    };
    let mut notes = Vec::new();
    if line_filtered(f) {
        notes.push("Expenses, inventory purchases and stock adjustments are business-wide, so they're left out while a staff, profile, service, category or product filter is active.".into());
    }
    Ok(Report { filter: f.clone(), summary, prior, prior_from, prior_to, groups: g, trend: trend_pts, notes })
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct TaxRow {
    pub month: String,
    pub rate: String,
    pub taxable_sales: Decimal,
    pub exempt_sales: Decimal,
    pub tax_collected: Decimal,
    pub tax_refunded: Decimal,
    pub net_tax: Decimal,
}

/// Sales tax by month and rate, for filing reference. Taxable/exempt amounts are pre-tax sales
/// (tips included under their own taxability); refunds count in the month they were issued.
pub fn tax_report(conn: &Connection, f: &ReportFilter) -> AppResult<Vec<TaxRow>> {
    let lines = line_facts(conn, f)?;
    let refunds = refund_facts(conn, f)?;
    let mut map: BTreeMap<(String, String), TaxRow> = BTreeMap::new();
    for l in &lines {
        let row = map.entry((l.sale_date[..7].to_string(), l.tax_label.clone())).or_default();
        if l.taxability == "taxable" {
            row.taxable_sales += l.net;
        } else {
            row.exempt_sales += l.net;
        }
        row.tax_collected += l.tax;
    }
    for r in &refunds {
        // Refunds don't carry the rate label; they're attributed to the month they were issued.
        let label = lines.iter().find(|l| l.sale_id == r.sale_id).map(|l| l.tax_label.clone()).unwrap_or_default();
        map.entry((r.refund_date[..7].to_string(), label)).or_default().tax_refunded += r.tax;
    }
    Ok(map
        .into_iter()
        .map(|((m, rate), mut row)| {
            row.month = m;
            row.rate = rate;
            row.net_tax = row.tax_collected - row.tax_refunded;
            row
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::sales::tests::{draft, fixture, resolve_tax_setup};
    use crate::db::sales::{add_payment, finalize, refund, save_draft, PaymentInput, RefundInput, RefundLineInput};
    use rust_decimal_macros::dec;

    fn filter() -> ReportFilter {
        ReportFilter { from: "2026-10-01".into(), to: "2026-10-31".into(), ..Default::default() }
    }

    #[test]
    fn totals_reconcile_with_groups_and_records() {
        let f = fixture();
        resolve_tax_setup(&f);
        let mut ids = vec![];
        for _ in 0..3 {
            let id = save_draft(&f.conn, &draft(&f)).unwrap();
            let v = finalize(&f.conn, id).unwrap();
            add_payment(&f.conn, id, &PaymentInput { method: "card".into(), amount: v.totals.total, reference: String::new(), paid_on: "2026-10-05".into() }).unwrap();
            ids.push((id, v));
        }
        let (id, v) = &ids[0];
        refund(&f.conn, *id, &RefundInput { refund_date: "2026-10-06".into(), reason: "x".into(), method: "card".into(), lines: vec![RefundLineInput { sale_line_id: v.lines[1].id, qty: dec!(1), restock: true }] }).unwrap();
        crate::db::expenses::save(&f.conn, &crate::db::expenses::ExpenseInput { id: None, expense_date: "2026-10-02".into(), category: "rent".into(), vendor: String::new(), description: String::new(), amount: dec!(1000), payment_method: String::new(), location_id: None, attachment_id: None }).unwrap();

        let r = report(&f.conn, &filter(), &["service".into(), "staff".into()], Some("week"), true).unwrap();
        let s = &r.summary;
        assert_eq!(s.sales_count, 3);
        assert_eq!(s.service_sales, dec!(285));
        assert_eq!(s.retail_sales, dec!(144));
        assert_eq!(s.refunds, dec!(24));
        assert_eq!(s.tips, dec!(60));
        assert_eq!(s.net_revenue, dec!(405));
        // tax: 3 × 14.30 − 2.40 refunded
        assert_eq!(s.tax_collected, dec!(40.5));
        // groups reconcile with the summary
        let by_service = &r.groups["service"];
        let rev: Decimal = by_service.iter().map(|g| g.revenue - g.refunds).sum();
        assert_eq!(rev, s.net_revenue);
        let gp: Decimal = by_service.iter().map(|g| g.gross_profit).sum();
        assert_eq!(gp, s.gross_profit);
        let contrib: Decimal = r.groups["staff"].iter().map(|g| g.contribution).sum();
        assert_eq!(contrib, s.service_contribution);
        // trend reconciles too
        let t_rev: Decimal = r.trend.iter().map(|p| p.net_revenue).sum();
        assert_eq!(t_rev, s.net_revenue);
        let t_exp: Decimal = r.trend.iter().map(|p| p.expenses).sum();
        assert_eq!(t_exp, dec!(1000));
        // fees: card on every sale, allocated to lines exactly
        let fees_paid: Decimal = (1..=3).map(|i| crate::db::sales::payments(&f.conn, ids[i - 1].0).unwrap().iter().map(|p| p.fee).sum::<Decimal>()).sum();
        assert_eq!(s.processing_fees.round_dp(10), fees_paid);
        // stock consumed from the ledger equals materials used + net retail cost of goods
        assert_eq!(s.stock_consumed, s.materials_used + s.retail_cogs);
        // operating result subtracts actual expenses, not allocated overhead
        assert_eq!(s.operating_result, s.gross_profit - dec!(1000));
        assert!(r.prior.is_some());
        assert_eq!(r.prior.as_ref().unwrap().sales_count, 0);
    }

    #[test]
    fn voided_sales_are_excluded() {
        let f = fixture();
        resolve_tax_setup(&f);
        let id = save_draft(&f.conn, &draft(&f)).unwrap();
        finalize(&f.conn, id).unwrap();
        crate::db::sales::void(&f.conn, id, "mistake").unwrap();
        let r = report(&f.conn, &filter(), &[], None, false).unwrap();
        assert_eq!(r.summary.sales_count, 0);
        assert_eq!(r.summary.stock_consumed, dec!(0));
    }

    #[test]
    fn prior_range_is_same_length() {
        assert_eq!(prior_range("2026-10-01", "2026-10-31"), ("2026-08-31".into(), "2026-09-30".into()));
    }
}
