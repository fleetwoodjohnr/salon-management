//! Decimal helpers and the app-wide currency rounding rule.
//!
//! All intermediate arithmetic uses `rust_decimal` (28 significant digits, base-10, exact for
//! terminating decimals). Currency is rounded to cents, half away from zero, only when a figure
//! becomes a line or document total that a person will see or pay.

use crate::error::{AppError, AppResult};
use rust_decimal::{Decimal, RoundingStrategy};

pub const HUNDRED: Decimal = Decimal::ONE_HUNDRED;

/// Round to cents, half away from zero ($0.125 → $0.13, -$0.125 → -$0.13).
pub fn round_money(d: Decimal) -> Decimal {
    d.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// Percent (e.g. 35 for 35%) to a fraction (0.35).
pub fn frac(pct: Decimal) -> Decimal {
    pct / HUNDRED
}

pub fn non_negative(field: &str, label: &str, d: Decimal) -> AppResult<Decimal> {
    if d.is_sign_negative() && !d.is_zero() {
        return Err(AppError::invalid(field, format!("{label} cannot be negative.")));
    }
    Ok(d)
}

/// A percentage in [0, 100) — 100% or more would leave nothing (or less than nothing) of the price.
pub fn pct_below_100(field: &str, label: &str, d: Decimal) -> AppResult<Decimal> {
    non_negative(field, label, d)?;
    if d >= HUNDRED {
        return Err(AppError::invalid(field, format!("{label} must be less than 100%.")));
    }
    Ok(d)
}

pub fn pct_0_100(field: &str, label: &str, d: Decimal) -> AppResult<Decimal> {
    non_negative(field, label, d)?;
    if d > HUNDRED {
        return Err(AppError::invalid(field, format!("{label} cannot exceed 100%.")));
    }
    Ok(d)
}

/// Normalize for display/storage: strip trailing zeros ("1.500" → "1.5").
pub fn norm(d: Decimal) -> Decimal {
    d.normalize()
}

/// Split `extra` (in cents, can be negative) across weights so the parts sum exactly.
pub fn allocate(extra: Decimal, weights: &[Decimal]) -> Vec<Decimal> {
    let n = weights.len();
    if n == 0 || extra.is_zero() {
        return vec![Decimal::ZERO; n];
    }
    let total_w: Decimal = weights.iter().sum();
    let w: Vec<Decimal> = if total_w.is_zero() { vec![Decimal::ONE; n] } else { weights.to_vec() };
    let tw: Decimal = w.iter().sum();
    let cents = (extra.abs() * Decimal::ONE_HUNDRED).round();
    let exact: Vec<Decimal> = w.iter().map(|x| cents * x / tw).collect();
    let mut parts: Vec<Decimal> = exact.iter().map(|e| e.floor()).collect();
    let mut left = cents - parts.iter().sum::<Decimal>();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| (exact[b] - parts[b]).cmp(&(exact[a] - parts[a])).then(a.cmp(&b)));
    for i in order {
        if left <= Decimal::ZERO {
            break;
        }
        parts[i] += Decimal::ONE;
        left -= Decimal::ONE;
    }
    let sign = if extra < Decimal::ZERO { -Decimal::ONE } else { Decimal::ONE };
    parts.into_iter().map(|c| sign * c / Decimal::ONE_HUNDRED).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn rounding_is_half_away_from_zero() {
        assert_eq!(round_money(dec!(0.125)), dec!(0.13));
        assert_eq!(round_money(dec!(0.124999)), dec!(0.12));
        assert_eq!(round_money(dec!(-0.125)), dec!(-0.13));
        assert_eq!(round_money(dec!(2.675)), dec!(2.68)); // binary floats famously get this wrong
    }

    #[test]
    fn allocation_sums_exactly() {
        let shares = allocate(dec!(10.00), &[dec!(1), dec!(1), dec!(1)]);
        assert_eq!(shares.iter().sum::<Decimal>(), dec!(10.00));
        assert_eq!(shares, vec![dec!(3.34), dec!(3.33), dec!(3.33)]);
        let neg = allocate(dec!(-5.00), &[dec!(30), dec!(10)]);
        assert_eq!(neg, vec![dec!(-3.75), dec!(-1.25)]);
        assert_eq!(allocate(dec!(1), &[dec!(0), dec!(0)]), vec![dec!(0.50), dec!(0.50)]);
    }

    #[test]
    fn percent_validation() {
        assert!(pct_below_100("x", "X", dec!(100)).is_err());
        assert!(pct_below_100("x", "X", dec!(-1)).is_err());
        assert_eq!(pct_below_100("x", "X", dec!(99.5)).unwrap(), dec!(99.5));
        assert_eq!(frac(dec!(35)), dec!(0.35));
    }
}
