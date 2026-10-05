//! Deterministic pricing. Every price-dependent cost is linear in the list price L, so target prices
//! are solved in closed form — no iteration, no provisional price.
//!
//! With tax rate t (as a fraction, 0 if not taxable):
//!   tax-exclusive pricing: revenue R = L,          customer pays T = L(1+t)
//!   tax-inclusive pricing: revenue R = L ÷ (1+t),  customer pays T = L
//! Price-dependent costs: commission c·R (commission is on the pre-tax service price) and card
//! processing s·(p·T + p₀) where s is the share paid by card.
//!   Cost(L)   = F + s·p₀ + c·R + s·p·T          (F = materials, waste, other direct, labor, overhead)
//!   Profit(L) = R − Cost(L)
//!   Margin    = Profit ÷ R          Markup = Profit ÷ Cost
//! Target margin m:  L = (F + s·p₀) ÷ (α(1 − c − m) − s·p·β)
//! Target markup k:  L = (1+k)(F + s·p₀) ÷ (α − (1+k)(c·α + s·p·β))
//! where R = αL and T = βL. Break-even is the margin formula with m = 0. A denominator ≤ 0 means
//! the percentages alone consume the whole price, so no price can reach the target.

use super::money::{frac, round_money, HUNDRED};
use super::profile::{RoundMode, TargetKind};
use crate::error::{AppError, AppResult};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PriceParams {
    /// Costs that don't depend on price (materials, waste, other direct, labor, overhead).
    pub fixed_cost: Decimal,
    pub commission_pct: Decimal,
    pub processing_pct: Decimal,
    pub processing_fixed: Decimal,
    pub card_share_pct: Decimal,
    /// Combined sales-tax rate % when the service is taxable and the rate is known; Some(0) when
    /// exempt; None when unknown (processing on tax can't be included — flagged).
    pub tax_rate_pct: Option<Decimal>,
    pub prices_include_tax: bool,
    pub target_kind: TargetKind,
    pub target_pct: Decimal,
    pub rounding_increment: Decimal,
    pub rounding_mode: RoundMode,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct PriceAnalysis {
    pub list_price: Decimal,
    /// Price before tax (the business's revenue)
    pub revenue: Decimal,
    pub tax: Decimal,
    pub customer_total: Decimal,
    pub commission: Decimal,
    pub processing: Decimal,
    pub fixed_cost: Decimal,
    pub total_cost: Decimal,
    pub profit: Decimal,
    /// None when revenue is zero
    pub margin_pct: Option<Decimal>,
    pub markup_pct: Option<Decimal>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Targets {
    /// Lowest list price (to the cent, rounded up) that covers every cost.
    pub break_even: Decimal,
    /// Lowest list price (to the cent, rounded up) that reaches the target.
    pub target_price: Decimal,
    /// The target price after the rounding rule.
    pub target_rounded: Decimal,
    pub target_rounded_misses: bool,
}

struct Linear {
    alpha: Decimal,
    beta: Decimal,
    c: Decimal,
    sp: Decimal,
    fixed: Decimal,
}

impl PriceParams {
    pub fn validate(&self) -> AppResult<()> {
        super::money::non_negative("fixed_cost", "Cost", self.fixed_cost)?;
        super::money::pct_below_100("commission_pct", "Commission", self.commission_pct)?;
        super::money::pct_below_100("processing_pct", "Processing", self.processing_pct)?;
        super::money::non_negative("processing_fixed", "Processing fee", self.processing_fixed)?;
        super::money::pct_0_100("card_share_pct", "Card share", self.card_share_pct)?;
        if let Some(t) = self.tax_rate_pct {
            super::money::pct_below_100("tax_rate_pct", "Tax rate", t)?;
        }
        match self.target_kind {
            TargetKind::Margin => super::money::pct_below_100("target_pct", "Target margin", self.target_pct)?,
            TargetKind::Markup => super::money::non_negative("target_pct", "Target markup", self.target_pct)?,
        };
        if self.rounding_increment < Decimal::new(1, 2) {
            return Err(AppError::invalid("rounding_increment", "Rounding increment must be at least $0.01."));
        }
        Ok(())
    }

    fn linear(&self) -> Linear {
        let t = frac(self.tax_rate_pct.unwrap_or(Decimal::ZERO));
        let (alpha, beta) = if self.prices_include_tax { (Decimal::ONE / (Decimal::ONE + t), Decimal::ONE) } else { (Decimal::ONE, Decimal::ONE + t) };
        let s = frac(self.card_share_pct);
        Linear { alpha, beta, c: frac(self.commission_pct), sp: s * frac(self.processing_pct), fixed: self.fixed_cost + s * self.processing_fixed }
    }

    pub fn analyze(&self, list_price: Decimal) -> PriceAnalysis {
        let l = self.linear();
        let t = frac(self.tax_rate_pct.unwrap_or(Decimal::ZERO));
        // Tax and revenue are what a receipt would show: tax rounded to cents.
        let (revenue, tax) = if self.prices_include_tax {
            let tax = round_money(list_price * t / (Decimal::ONE + t));
            (list_price - tax, tax)
        } else {
            (list_price, round_money(list_price * t))
        };
        let customer_total = revenue + tax;
        let commission = l.c * revenue;
        let s = frac(self.card_share_pct);
        let processing = s * (frac(self.processing_pct) * customer_total + self.processing_fixed);
        let total_cost = self.fixed_cost + commission + processing;
        let profit = revenue - total_cost;
        PriceAnalysis {
            list_price,
            revenue,
            tax,
            customer_total,
            commission,
            processing,
            fixed_cost: self.fixed_cost,
            total_cost,
            profit,
            margin_pct: (!revenue.is_zero()).then(|| profit / revenue * HUNDRED),
            markup_pct: (!total_cost.is_zero()).then(|| profit / total_cost * HUNDRED),
        }
    }

    fn margin_price(&self, m: Decimal) -> AppResult<Decimal> {
        let l = self.linear();
        let denom = l.alpha * (Decimal::ONE - l.c - m) - l.sp * l.beta;
        if denom <= Decimal::ZERO {
            return Err(self.impossible(m));
        }
        Ok(l.fixed / denom)
    }

    fn impossible(&self, m: Decimal) -> AppError {
        AppError::invalid(
            "target_pct",
            format!(
                "No price can reach this target: commission ({}%), card processing ({}% of the card share, including tax) and the target ({}%) add up to the whole price. Lower one of them.",
                self.commission_pct.normalize(),
                self.processing_pct.normalize(),
                (m * HUNDRED).normalize()
            ),
        )
    }

    pub fn round(&self, price: Decimal) -> Decimal {
        let inc = self.rounding_increment;
        let steps = price / inc;
        let n = match self.rounding_mode {
            RoundMode::Up => steps.ceil(),
            RoundMode::Down => steps.floor(),
            RoundMode::Nearest => steps.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero),
        };
        round_money(n * inc)
    }

    pub fn targets(&self) -> AppResult<Targets> {
        self.validate()?;
        // Round up to the cent, after dropping division residue beyond 10 dp (66.000…0001 is 66).
        let up = |d: Decimal| d.round_dp(10).round_dp_with_strategy(2, RoundingStrategy::AwayFromZero);
        let break_even = up(self.margin_price(Decimal::ZERO)?);
        let raw = match self.target_kind {
            TargetKind::Margin => self.margin_price(frac(self.target_pct))?,
            TargetKind::Markup => {
                let l = self.linear();
                let k = frac(self.target_pct);
                let denom = l.alpha - (Decimal::ONE + k) * (l.c * l.alpha + l.sp * l.beta);
                if denom <= Decimal::ZERO {
                    return Err(self.impossible(Decimal::ZERO));
                }
                (Decimal::ONE + k) * l.fixed / denom
            }
        };
        let target_price = up(raw);
        let target_rounded = self.round(target_price);
        Ok(Targets {
            break_even,
            target_price,
            target_rounded,
            target_rounded_misses: !self.meets_target(target_rounded),
        })
    }

    /// Does `price` reach the target (margin or markup) once tax is rounded as on a receipt?
    pub fn meets_target(&self, price: Decimal) -> bool {
        let a = self.analyze(price);
        let tolerance = Decimal::new(1, 4); // 0.0001 percentage points: receipt rounding noise
        match self.target_kind {
            TargetKind::Margin => a.margin_pct.is_some_and(|m| m + tolerance >= self.target_pct),
            TargetKind::Markup => a.markup_pct.is_some_and(|m| m + tolerance >= self.target_pct) || (a.total_cost.is_zero() && a.profit >= Decimal::ZERO),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn simple(kind: TargetKind, pct: Decimal) -> PriceParams {
        PriceParams {
            fixed_cost: dec!(60),
            commission_pct: dec!(0),
            processing_pct: dec!(0),
            processing_fixed: dec!(0),
            card_share_pct: dec!(100),
            tax_rate_pct: Some(dec!(0)),
            prices_include_tax: false,
            target_kind: kind,
            target_pct: pct,
            rounding_increment: dec!(0.01),
            rounding_mode: RoundMode::Up,
        }
    }

    #[test]
    fn margin_versus_markup() {
        // cost ÷ (1 − margin): 60 ÷ 0.6 = 100
        assert_eq!(simple(TargetKind::Margin, dec!(40)).targets().unwrap().target_price, dec!(100));
        // cost × (1 + markup): 60 × 1.4 = 84
        assert_eq!(simple(TargetKind::Markup, dec!(40)).targets().unwrap().target_price, dec!(84));
        let a = simple(TargetKind::Margin, dec!(40)).analyze(dec!(100));
        assert_eq!(a.margin_pct.unwrap(), dec!(40));
        assert_eq!(a.markup_pct.unwrap().round_dp(2), dec!(66.67));
    }

    #[test]
    fn commission_is_solved_not_estimated() {
        let mut p = simple(TargetKind::Margin, dec!(20));
        p.commission_pct = dec!(40);
        let t = p.targets().unwrap();
        // 60 ÷ (1 − 0.4 − 0.2) = 150
        assert_eq!(t.target_price, dec!(150));
        let a = p.analyze(dec!(150));
        assert_eq!(a.commission, dec!(60));
        assert_eq!(a.margin_pct.unwrap(), dec!(20)); // consistent at the recommended price
        assert_eq!(t.break_even, dec!(100));
    }

    #[test]
    fn processing_on_tax_inclusive_total() {
        let mut p = simple(TargetKind::Margin, dec!(20));
        p.processing_pct = dec!(3);
        p.processing_fixed = dec!(0.30);
        p.tax_rate_pct = Some(dec!(10));
        let t = p.targets().unwrap();
        // L = 60.30 ÷ (0.8 − 0.03 × 1.1) = 60.30 ÷ 0.767 = 78.617992… → 78.62
        assert_eq!(t.target_price, dec!(78.62));
        assert!(p.meets_target(t.target_price));
        assert!(!p.meets_target(dec!(78.50)));
        let a = p.analyze(dec!(78.62));
        assert_eq!(a.tax, dec!(7.86));
        assert_eq!(a.customer_total, dec!(86.48));
    }

    #[test]
    fn tax_inclusive_pricing() {
        let mut p = simple(TargetKind::Margin, dec!(0));
        p.prices_include_tax = true;
        p.tax_rate_pct = Some(dec!(10));
        // revenue must be 60, so list price 66
        assert_eq!(p.targets().unwrap().break_even, dec!(66));
        let a = p.analyze(dec!(66));
        assert_eq!(a.tax, dec!(6));
        assert_eq!(a.revenue, dec!(60));
        assert_eq!(a.customer_total, dec!(66));
    }

    #[test]
    fn impossible_targets() {
        let mut p = simple(TargetKind::Margin, dec!(50));
        p.commission_pct = dec!(45);
        p.processing_pct = dec!(5);
        let e = p.targets().unwrap_err().to_string();
        assert!(e.contains("No price can reach"), "{e}");
        p.target_pct = dec!(100);
        assert!(p.targets().is_err());
    }

    #[test]
    fn rounding_and_recalculated_margin() {
        let mut p = simple(TargetKind::Margin, dec!(40));
        p.fixed_cost = dec!(61);
        p.rounding_increment = dec!(5);
        let t = p.targets().unwrap(); // 101.666… → 101.67 → up to 105
        assert_eq!(t.target_price, dec!(101.67));
        assert_eq!(t.target_rounded, dec!(105));
        assert!(!t.target_rounded_misses);
        p.rounding_mode = RoundMode::Down;
        let t = p.targets().unwrap();
        assert_eq!(t.target_rounded, dec!(100));
        assert!(t.target_rounded_misses); // rounding down misses the target — flagged
        p.rounding_mode = RoundMode::Nearest;
        assert_eq!(p.round(dec!(102.5)), dec!(105));
        assert_eq!(p.round(dec!(102.49)), dec!(100));
    }
}
