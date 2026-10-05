//! Sale arithmetic: line totals, discount allocation, sales tax and refunds. Pure functions.
//!
//! - Line gross = round(qty × unit price) − line discount.
//! - A sale-level discount is split across service and retail lines in proportion to their gross
//!   (to the cent, largest remainder). Tips are never discounted.
//! - Tax is computed once per sale on the summed taxable amount and rounded once (half away from
//!   zero), then split across taxable lines in proportion to their amounts so refunds can return the
//!   right share. Tax-exclusive: tax = base × rate, added on top. Tax-inclusive: tax = base × rate ÷
//!   (1 + rate), already inside the price.
//! - Any line whose taxability is unknown, or a taxable line with no rate, makes the sale's tax
//!   unresolved: the figure shown is an estimate and the sale can't be finalized.

use super::money::{allocate, frac, round_money};
use crate::error::{AppError, AppResult};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LineKind {
    Service,
    Retail,
    Tip,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Taxability {
    Taxable,
    Exempt,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct LineIn {
    pub kind: LineKind,
    pub qty: Decimal,
    pub unit_price: Decimal,
    pub line_discount: Decimal,
    pub taxability: Taxability,
    pub label: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct LineOut {
    pub gross: Decimal,
    pub sale_discount_share: Decimal,
    /// Pre-tax revenue for the line (for tips: the tip amount)
    pub net: Decimal,
    pub tax: Decimal,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct SaleOut {
    pub lines: Vec<LineOut>,
    /// Σ gross of service + retail lines (after line discounts, before the sale discount)
    pub subtotal: Decimal,
    pub discount_total: Decimal,
    pub tax_total: Decimal,
    pub tip_total: Decimal,
    /// What the client pays (including tips)
    pub total: Decimal,
    pub tax_resolved: bool,
    pub unresolved: Vec<String>,
}

pub fn compute(lines: &[LineIn], sale_discount: Decimal, tax_rate_pct: Option<Decimal>, prices_include_tax: bool) -> AppResult<SaleOut> {
    if sale_discount < Decimal::ZERO {
        return Err(AppError::invalid("sale_discount", "Discount can't be negative."));
    }
    let mut gross = Vec::with_capacity(lines.len());
    let mut line_discounts = Decimal::ZERO;
    for (i, l) in lines.iter().enumerate() {
        if l.qty <= Decimal::ZERO {
            return Err(AppError::invalid(&format!("lines.{i}.qty"), "Quantity must be more than zero."));
        }
        if l.unit_price < Decimal::ZERO {
            return Err(AppError::invalid(&format!("lines.{i}.unit_price"), "Price can't be negative."));
        }
        if l.line_discount < Decimal::ZERO {
            return Err(AppError::invalid(&format!("lines.{i}.line_discount"), "Discount can't be negative."));
        }
        if l.kind == LineKind::Tip && !l.line_discount.is_zero() {
            return Err(AppError::invalid(&format!("lines.{i}.line_discount"), "Tips can't be discounted."));
        }
        let ext = round_money(l.qty * l.unit_price);
        if l.line_discount > ext {
            return Err(AppError::invalid(&format!("lines.{i}.line_discount"), "Discount is larger than the line."));
        }
        line_discounts += l.line_discount;
        gross.push(ext - l.line_discount);
    }
    let discountable: Vec<Decimal> = lines.iter().zip(&gross).map(|(l, g)| if l.kind == LineKind::Tip { Decimal::ZERO } else { *g }).collect();
    let subtotal: Decimal = discountable.iter().sum();
    if sale_discount > subtotal {
        return Err(AppError::invalid("sale_discount", "The discount is larger than the sale."));
    }
    let shares = if subtotal.is_zero() { vec![Decimal::ZERO; lines.len()] } else { allocate(sale_discount, &discountable) };
    let after: Vec<Decimal> = gross.iter().zip(&shares).map(|(g, s)| g - s).collect();

    let mut unresolved = Vec::new();
    for l in lines {
        if l.taxability == Taxability::Unknown {
            unresolved.push(format!("Whether {} is taxable hasn't been decided.", l.label));
        }
    }
    let taxable_w: Vec<Decimal> = lines.iter().zip(&after).map(|(l, a)| if l.taxability == Taxability::Taxable { *a } else { Decimal::ZERO }).collect();
    let base: Decimal = taxable_w.iter().sum();
    if !base.is_zero() && tax_rate_pct.is_none() {
        unresolved.push("No sales tax rate is set for this location.".into());
    }
    let r = frac(tax_rate_pct.unwrap_or(Decimal::ZERO));
    let tax_total = if prices_include_tax { round_money(base * r / (Decimal::ONE + r)) } else { round_money(base * r) };
    let taxes = if base.is_zero() { vec![Decimal::ZERO; lines.len()] } else { allocate(tax_total, &taxable_w) };

    let out: Vec<LineOut> = (0..lines.len())
        .map(|i| LineOut {
            gross: gross[i],
            sale_discount_share: shares[i],
            net: if prices_include_tax { after[i] - taxes[i] } else { after[i] },
            tax: taxes[i],
        })
        .collect();
    let tip_total: Decimal = lines.iter().zip(&after).filter(|(l, _)| l.kind == LineKind::Tip).map(|(_, a)| *a).sum();
    let all_after: Decimal = after.iter().sum();
    let total = if prices_include_tax { all_after } else { all_after + tax_total };
    Ok(SaleOut {
        lines: out,
        subtotal,
        discount_total: line_discounts + sale_discount,
        tax_total,
        tip_total,
        total,
        tax_resolved: unresolved.is_empty(),
        unresolved,
    })
}

/// Share of a finalized line to refund. `already` is what earlier refunds returned for this line;
/// the last refund takes exactly what's left so rounding never leaves stray cents.
pub fn refund_share(line_qty: Decimal, line_net: Decimal, line_tax: Decimal, already_qty: Decimal, already_net: Decimal, already_tax: Decimal, qty: Decimal) -> AppResult<(Decimal, Decimal)> {
    if qty <= Decimal::ZERO {
        return Err(AppError::invalid("qty", "Refund quantity must be more than zero."));
    }
    let remaining = line_qty - already_qty;
    if qty > remaining {
        return Err(AppError::invalid("qty", format!("Only {} left to refund on this line.", remaining.normalize())));
    }
    if qty == remaining {
        return Ok((line_net - already_net, line_tax - already_tax));
    }
    Ok((round_money(line_net * qty / line_qty), round_money(line_tax * qty / line_qty)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn line(kind: LineKind, qty: Decimal, price: Decimal, disc: Decimal, t: Taxability) -> LineIn {
        LineIn { kind, qty, unit_price: price, line_discount: disc, taxability: t, label: "x".into() }
    }

    #[test]
    fn exclusive_tax_with_discounts_and_tip() {
        let lines = [
            line(LineKind::Service, dec!(1), dec!(85), dec!(0), Taxability::Taxable),
            line(LineKind::Retail, dec!(2), dec!(14.99), dec!(2), Taxability::Taxable),
            line(LineKind::Tip, dec!(1), dec!(15), dec!(0), Taxability::Exempt),
        ];
        let s = compute(&lines, dec!(10), Some(dec!(9.8)), false).unwrap();
        assert_eq!(s.subtotal, dec!(112.98)); // 85 + 29.98 − 2
        assert_eq!(s.discount_total, dec!(12));
        // discount split 85 : 27.98 → 7.52 / 2.48
        assert_eq!(s.lines[0].sale_discount_share + s.lines[1].sale_discount_share, dec!(10));
        assert_eq!(s.lines[2].sale_discount_share, dec!(0));
        // taxable base 102.98 × 9.8% = 10.09204 → 10.09, rounded once
        assert_eq!(s.tax_total, dec!(10.09));
        assert_eq!(s.lines[0].tax + s.lines[1].tax, dec!(10.09));
        assert_eq!(s.tip_total, dec!(15));
        assert_eq!(s.total, dec!(102.98) + dec!(10.09) + dec!(15));
        assert!(s.tax_resolved);
    }

    #[test]
    fn inclusive_tax() {
        let s = compute(&[line(LineKind::Service, dec!(1), dec!(110), dec!(0), Taxability::Taxable)], dec!(0), Some(dec!(10)), true).unwrap();
        assert_eq!(s.tax_total, dec!(10));
        assert_eq!(s.lines[0].net, dec!(100));
        assert_eq!(s.total, dec!(110));
    }

    #[test]
    fn unknown_taxability_and_missing_rate_are_unresolved_not_zero() {
        let s = compute(&[line(LineKind::Service, dec!(1), dec!(50), dec!(0), Taxability::Unknown)], dec!(0), Some(dec!(8)), false).unwrap();
        assert!(!s.tax_resolved);
        let s = compute(&[line(LineKind::Service, dec!(1), dec!(50), dec!(0), Taxability::Taxable)], dec!(0), None, false).unwrap();
        assert!(!s.tax_resolved);
        assert!(s.unresolved[0].contains("No sales tax rate"));
        // exempt sale with no rate is fine
        let s = compute(&[line(LineKind::Service, dec!(1), dec!(50), dec!(0), Taxability::Exempt)], dec!(0), None, false).unwrap();
        assert!(s.tax_resolved);
        assert_eq!(s.tax_total, dec!(0));
    }

    #[test]
    fn validation() {
        assert!(compute(&[line(LineKind::Service, dec!(1), dec!(10), dec!(11), Taxability::Exempt)], dec!(0), None, false).is_err());
        assert!(compute(&[line(LineKind::Service, dec!(1), dec!(10), dec!(0), Taxability::Exempt)], dec!(11), None, false).is_err());
        assert!(compute(&[line(LineKind::Tip, dec!(1), dec!(10), dec!(1), Taxability::Exempt)], dec!(0), None, false).is_err());
    }

    #[test]
    fn partial_refunds_never_drift() {
        // 3 units, net 10.00, tax 0.98
        let (n1, t1) = refund_share(dec!(3), dec!(10), dec!(0.98), dec!(0), dec!(0), dec!(0), dec!(1)).unwrap();
        assert_eq!((n1, t1), (dec!(3.33), dec!(0.33)));
        let (n2, t2) = refund_share(dec!(3), dec!(10), dec!(0.98), dec!(1), n1, t1, dec!(1)).unwrap();
        let (n3, t3) = refund_share(dec!(3), dec!(10), dec!(0.98), dec!(2), n1 + n2, t1 + t2, dec!(1)).unwrap();
        assert_eq!(n1 + n2 + n3, dec!(10));
        assert_eq!(t1 + t2 + t3, dec!(0.98));
        assert!(refund_share(dec!(3), dec!(10), dec!(0.98), dec!(3), dec!(10), dec!(0.98), dec!(1)).is_err());
    }
}
