//! Cost of performing one service, before any price-dependent fees.
//!
//! - Materials: recipe quantities × current moving-average cost (estimates) or the cost frozen on the
//!   ledger when material was actually used (completed sales).
//! - Waste: estimated extra material, materials × waste %, shown separately.
//! - Labor: working hours (hands-on + setup + cleanup) × pay per billable hour. Processing time is not
//!   labor: the professional is free to do other work while color processes.
//! - Overhead: hours × overhead per billable hour, where hours are either all chair time
//!   (hands-on + processing + setup + cleanup) or working time only, per the profile's choice.

use super::money::frac;
use super::profile::{OverheadBasis, ProfileRates};
use crate::error::{AppError, AppResult};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct TimeSpec {
    pub hands_on_min: i64,
    pub processing_min: i64,
    pub setup_min: i64,
    pub cleanup_min: i64,
}

impl TimeSpec {
    pub fn validate(&self) -> AppResult<()> {
        for (f, v) in [
            ("hands_on_min", self.hands_on_min),
            ("processing_min", self.processing_min),
            ("setup_min", self.setup_min),
            ("cleanup_min", self.cleanup_min),
        ] {
            if !(0..=24 * 60).contains(&v) {
                return Err(AppError::invalid(f, "Minutes must be between 0 and 1440."));
            }
        }
        if self.hands_on_min + self.processing_min == 0 {
            return Err(AppError::invalid("hands_on_min", "A service needs some hands-on or processing time."));
        }
        Ok(())
    }
    pub fn working_min(&self) -> i64 {
        self.hands_on_min + self.setup_min + self.cleanup_min
    }
    pub fn occupied_min(&self) -> i64 {
        self.working_min() + self.processing_min
    }
    pub fn add(&self, o: &TimeSpec) -> TimeSpec {
        TimeSpec {
            hands_on_min: self.hands_on_min + o.hands_on_min,
            processing_min: self.processing_min + o.processing_min,
            setup_min: self.setup_min + o.setup_min,
            cleanup_min: self.cleanup_min + o.cleanup_min,
        }
    }
}

pub fn hours(min: i64) -> Decimal {
    Decimal::from(min) / Decimal::from(60)
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct MaterialCost {
    pub product_id: i64,
    pub name: String,
    /// Quantity as entered (in `unit`)
    pub qty: Decimal,
    pub unit: String,
    pub qty_base: Decimal,
    pub base_unit: String,
    /// Average cost per base unit; None when the product has never been costed
    pub unit_cost: Option<Decimal>,
    pub cost: Decimal,
    pub warning: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct CostBreakdown {
    pub materials: Decimal,
    pub waste: Decimal,
    pub other_direct: Decimal,
    pub labor: Decimal,
    pub overhead: Decimal,
    /// materials + waste + other direct + labor + overhead (costs that don't depend on price)
    pub fixed_total: Decimal,
    pub time: TimeSpec,
    pub working_hours: Decimal,
    pub occupied_hours: Decimal,
    pub overhead_hours: Decimal,
    pub labor_rate: Decimal,
    pub overhead_rate: Decimal,
    pub lines: Vec<MaterialCost>,
    pub warnings: Vec<String>,
}

pub fn service_cost(
    rates: &ProfileRates,
    basis: OverheadBasis,
    time: TimeSpec,
    lines: Vec<MaterialCost>,
    waste_pct: Decimal,
    other_direct: Decimal,
) -> AppResult<CostBreakdown> {
    time.validate()?;
    super::money::pct_0_100("waste_pct", "Waste allowance", waste_pct)?;
    super::money::non_negative("other_direct_cost", "Other direct cost", other_direct)?;
    let materials: Decimal = lines.iter().map(|l| l.cost).sum();
    let waste = materials * frac(waste_pct);
    let working_hours = hours(time.working_min());
    let occupied_hours = hours(time.occupied_min());
    let overhead_hours = match basis {
        OverheadBasis::Occupied => occupied_hours,
        OverheadBasis::HandsOn => working_hours,
    };
    let labor = working_hours * rates.labor_cost_per_billable_hour;
    let overhead = overhead_hours * rates.overhead_per_billable_hour;
    let mut warnings: Vec<String> = lines.iter().filter_map(|l| l.warning.clone()).collect();
    if lines.is_empty() {
        warnings.push("No recipe yet: material cost is $0 until you add the products this service uses.".into());
    }
    Ok(CostBreakdown {
        materials,
        waste,
        other_direct,
        labor,
        overhead,
        fixed_total: materials + waste + other_direct + labor + overhead,
        time,
        working_hours,
        occupied_hours,
        overhead_hours,
        labor_rate: rates.labor_cost_per_billable_hour,
        overhead_rate: rates.overhead_per_billable_hour,
        lines,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::profile::sample;
    use rust_decimal_macros::dec;

    fn line(cost: Decimal) -> MaterialCost {
        MaterialCost {
            product_id: 1,
            name: "Developer".into(),
            qty: dec!(2),
            unit: "fl_oz".into(),
            qty_base: dec!(59.147059125),
            base_unit: "ml".into(),
            unit_cost: None,
            cost,
            warning: None,
        }
    }

    #[test]
    fn processing_time_is_overhead_not_labor() {
        let rates = sample().rates().unwrap(); // $40 labor, $12.666… overhead per billable hour
        let t = TimeSpec { hands_on_min: 60, processing_min: 30, setup_min: 0, cleanup_min: 0 };
        let b = service_cost(&rates, OverheadBasis::Occupied, t, vec![line(dec!(1.50))], dec!(10), dec!(0.5)).unwrap();
        assert_eq!(b.labor, dec!(40));
        assert_eq!(b.overhead.round_dp(2), dec!(19.00)); // 1.5 h × 12.666…
        assert_eq!(b.waste, dec!(0.15));
        let b2 = service_cost(&rates, OverheadBasis::HandsOn, t, vec![line(dec!(1.50))], dec!(10), dec!(0.5)).unwrap();
        assert_eq!(b2.overhead.round_dp(2), dec!(12.67));
        assert_eq!(b.fixed_total.round_dp(2), dec!(61.15));
    }

    #[test]
    fn invalid_time() {
        let rates = sample().rates().unwrap();
        let t = TimeSpec::default();
        assert!(service_cost(&rates, OverheadBasis::Occupied, t, vec![], dec!(0), dec!(0)).is_err());
    }
}
