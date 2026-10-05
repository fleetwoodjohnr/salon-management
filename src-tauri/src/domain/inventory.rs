//! Inventory valuation: moving weighted average, tracked as on-hand quantity Q (base units) and
//! on-hand value V for each product.
//!
//! - Receiving q at cost c: Q += q, V += c.
//! - Issuing q (service use, retail sale, waste, supplier return, negative count adjustment):
//!   cost = V × q ÷ Q (multiply first, so terminating results are exact). Issuing everything that is
//!   on hand costs exactly V, so the value returns to zero with no rounding residue.
//! - Issuing more than is on hand: the excess is costed at the last known average and stock goes
//!   negative (with a warning). When stock arrives while negative, the remaining quantity is valued at
//!   the new purchase cost and the difference is posted as a revaluation entry, so the ledger always
//!   sums to the current value.
//! - The cost assigned to an issue is stored on its ledger row and never recalculated, so later
//!   purchases don't rewrite past profitability.
//! - Assigned costs are rounded to 10 decimal places (a ten-billionth of a dollar). Without that,
//!   28-digit arithmetic can round differently depending on the order of operations, and the
//!   ledger's sum could drift from the stored value by a few 10⁻²⁵ over thousands of movements.

use crate::error::{AppError, AppResult};
use rust_decimal::{Decimal, RoundingStrategy};

fn r10(d: Decimal) -> Decimal {
    d.round_dp_with_strategy(10, RoundingStrategy::MidpointAwayFromZero)
}
use serde::Serialize;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct Stock {
    pub qty: Decimal,
    pub value: Decimal,
    /// Average cost per base unit at the last point stock was positive (used for negative stock).
    pub last_unit_cost: Option<Decimal>,
}

#[derive(Debug, PartialEq)]
pub struct Movement {
    pub after: Stock,
    /// Value moved by this entry (positive in, negative out).
    pub value: Decimal,
    /// Extra value correction to post as a separate `revaluation` ledger row (usually zero).
    pub revaluation: Decimal,
    pub went_negative: bool,
}

impl Stock {
    /// Average cost per base unit, if known.
    pub fn avg_unit_cost(&self) -> Option<Decimal> {
        if self.qty > Decimal::ZERO {
            Some(self.value / self.qty)
        } else {
            self.last_unit_cost
        }
    }

    /// Cost of `qty` base units at the current average, without changing anything.
    pub fn cost_of(&self, qty: Decimal) -> Option<Decimal> {
        if self.qty > Decimal::ZERO {
            if qty >= self.qty {
                let extra = qty - self.qty;
                return Some(self.value + extra * self.value / self.qty);
            }
            Some(self.value * qty / self.qty)
        } else {
            self.last_unit_cost.map(|u| u * qty)
        }
    }

    pub fn receive(&self, qty: Decimal, cost: Decimal) -> AppResult<Movement> {
        if qty <= Decimal::ZERO {
            return Err(AppError::msg("Received quantity must be more than zero."));
        }
        let cost = r10(cost);
        if cost < Decimal::ZERO {
            return Err(AppError::msg("Cost can't be negative."));
        }
        let unit = cost / qty;
        let new_qty = self.qty + qty;
        let mut revaluation = Decimal::ZERO;
        let new_value = if self.qty < Decimal::ZERO && new_qty > Decimal::ZERO {
            // Stock was negative: what remains is valued at this purchase's cost.
            let target = r10(cost * new_qty / qty);
            revaluation = target - (self.value + cost);
            target
        } else if self.qty < Decimal::ZERO {
            // Still negative or exactly zero afterwards.
            let target = if new_qty.is_zero() { Decimal::ZERO } else { r10(unit * new_qty) };
            revaluation = target - (self.value + cost);
            target
        } else {
            self.value + cost
        };
        Ok(Movement {
            after: Stock { qty: new_qty, value: new_value, last_unit_cost: Some(if new_qty > Decimal::ZERO { new_value / new_qty } else { unit }) },
            value: cost,
            revaluation,
            went_negative: false,
        })
    }

    /// Take `qty` out at the moving average. Errors only when there is no cost basis at all.
    pub fn issue(&self, qty: Decimal) -> AppResult<Movement> {
        if qty <= Decimal::ZERO {
            return Err(AppError::msg("Quantity must be more than zero."));
        }
        let cost = self.cost_of(qty).map(r10).ok_or_else(|| {
            AppError::msg("This product has no purchase cost yet. Receive it (or record an opening count with a cost) before using it.")
        })?;
        let new_qty = self.qty - qty;
        let new_value = if new_qty.is_zero() { Decimal::ZERO } else { self.value - cost };
        // If we landed exactly on zero quantity the value must be zero too.
        let cost = if new_qty.is_zero() { self.value } else { cost };
        Ok(Movement {
            after: Stock { qty: new_qty, value: new_value, last_unit_cost: self.avg_unit_cost() },
            value: -cost,
            revaluation: Decimal::ZERO,
            went_negative: new_qty < Decimal::ZERO,
        })
    }

    /// Apply an exact negation of an earlier entry (a correction). If that leaves zero quantity
    /// with leftover value, the leftover is cleared by a revaluation.
    pub fn reverse(&self, entry_qty: Decimal, entry_value: Decimal) -> Movement {
        let new_qty = self.qty - entry_qty;
        let mut new_value = self.value - entry_value;
        let mut revaluation = Decimal::ZERO;
        if new_qty.is_zero() && !new_value.is_zero() {
            revaluation = -new_value;
            new_value = Decimal::ZERO;
        }
        let after = Stock {
            qty: new_qty,
            value: new_value,
            last_unit_cost: if new_qty > Decimal::ZERO { Some(new_value / new_qty) } else { self.last_unit_cost },
        };
        Movement { after, value: -entry_value, revaluation, went_negative: new_qty < Decimal::ZERO }
    }

    /// Count adjustment to an exact quantity. Gains are valued at the average cost (or `unit_cost`
    /// if given / if there is no average yet); losses are issued at the average.
    pub fn adjust_to(&self, counted: Decimal, unit_cost: Option<Decimal>) -> AppResult<Movement> {
        let delta = counted - self.qty;
        if delta.is_zero() {
            return Err(AppError::msg("The count matches what's on hand; nothing to adjust."));
        }
        if delta < Decimal::ZERO {
            return self.issue(-delta);
        }
        let unit = unit_cost.or(self.avg_unit_cost()).ok_or_else(|| {
            AppError::msg("Enter a cost per unit: this product has never been purchased, so there's no average cost to use.")
        })?;
        self.receive(delta, unit * delta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::units::{Dimension, ProductUnits};
    use rust_decimal_macros::dec;

    #[test]
    fn acceptance_32_fl_oz_bottle() {
        let pu = ProductUnits { dimension: Some(Dimension::Volume), ..Default::default() };
        let q = pu.to_base(dec!(32), "fl_oz").unwrap();
        let s = Stock::default().receive(q, dec!(24)).unwrap().after;
        // $0.75 per fl oz (multiply first: value × unit size ÷ quantity)
        assert_eq!(s.value * pu.to_base(dec!(1), "fl_oz").unwrap() / s.qty, dec!(0.75));
        let use_q = pu.to_base(dec!(2), "fl_oz").unwrap();
        let m = s.issue(use_q).unwrap();
        assert_eq!(m.value, dec!(-1.5));
        assert_eq!(pu.from_base(m.after.qty, "fl_oz").unwrap(), dec!(30));
        assert_eq!(m.after.value, dec!(22.5));
    }

    #[test]
    fn weighted_average_across_purchases_and_history_preserved() {
        let s = Stock::default().receive(dec!(10), dec!(10)).unwrap().after; // $1/unit
        let first_use = s.issue(dec!(4)).unwrap(); // costs $4
        assert_eq!(first_use.value, dec!(-4));
        let s = first_use.after.receive(dec!(6), dec!(12)).unwrap().after; // 6@$1 + 6@$2 = 12 for $18
        assert_eq!(s.qty, dec!(12));
        assert_eq!(s.value, dec!(18));
        assert_eq!(s.avg_unit_cost(), Some(dec!(1.5)));
        let second = s.issue(dec!(2)).unwrap();
        assert_eq!(second.value, dec!(-3));
        // the earlier issue's $4 is untouched by the later purchase (it was returned, not recomputed)
        assert_eq!(first_use.value, dec!(-4));
    }

    #[test]
    fn issuing_everything_clears_value_exactly() {
        let s = Stock::default().receive(dec!(3), dec!(10)).unwrap().after;
        let a = s.issue(dec!(1)).unwrap(); // 3.333…
        let b = a.after.issue(dec!(1)).unwrap();
        let c = b.after.issue(dec!(1)).unwrap();
        assert_eq!(c.after.qty, dec!(0));
        assert_eq!(c.after.value, dec!(0));
        assert_eq!(-(a.value + b.value + c.value), dec!(10));
    }

    #[test]
    fn negative_stock_then_purchase_revalues() {
        let s = Stock::default().receive(dec!(10), dec!(20)).unwrap().after; // $2/unit
        let over = s.issue(dec!(12)).unwrap(); // 2 more than on hand, at $2 → $24
        assert!(over.went_negative);
        assert_eq!(over.value, dec!(-24));
        assert_eq!(over.after.qty, dec!(-2));
        assert_eq!(over.after.value, dec!(-4));
        let r = over.after.receive(dec!(10), dec!(30)).unwrap(); // new cost $3/unit
        assert_eq!(r.after.qty, dec!(8));
        assert_eq!(r.after.value, dec!(24)); // remaining 8 valued at $3
        // ledger still sums: -4 + 30 + revaluation = 24
        assert_eq!(dec!(-4) + r.value + r.revaluation, dec!(24));
    }

    #[test]
    fn no_cost_basis_errors() {
        assert!(Stock::default().issue(dec!(1)).is_err());
        assert!(Stock::default().adjust_to(dec!(5), None).is_err());
        let m = Stock::default().adjust_to(dec!(5), Some(dec!(2))).unwrap();
        assert_eq!(m.after.value, dec!(10));
    }

    #[test]
    fn reversal_restores_and_clears_residue() {
        let s0 = Stock::default().receive(dec!(10), dec!(10)).unwrap().after;
        let s1 = s0.receive(dec!(10), dec!(20)).unwrap().after; // Q20 V30
        let used = s1.issue(dec!(10)).unwrap(); // $15
        let rev = used.after.reverse(dec!(10), dec!(20)); // undo the second purchase → Q0
        assert_eq!(rev.after.qty, dec!(0));
        assert_eq!(rev.after.value, dec!(0));
        assert_eq!(rev.revaluation, dec!(5));
    }
}
