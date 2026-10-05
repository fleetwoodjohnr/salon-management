//! Work profiles: how a professional's time and overhead are costed.
//!
//! Definitions (also in docs/calculations.md):
//! - Monthly hours = weekly hours × weeks worked per year ÷ 12.
//! - Billable hours = monthly hours × realistic utilization.
//! - Overhead per billable hour = monthly overhead ÷ billable hours.
//! - Labor cost per billable hour = hourly pay (plus employer burden for wages) ÷ utilization,
//!   because pay covers every hour worked but only billable hours can recover it.
//! - Commission is a percentage of the service price; it is never added on top of an hourly rate
//!   unless the compensation model is explicitly "hourly plus commission".

use super::money::{frac, non_negative, pct_0_100, pct_below_100, HUNDRED};
use crate::error::{AppError, AppResult};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileKind {
    Individual,
    ChairRenter,
    Independent,
    Employee,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CompModel {
    /// Owner/renter/independent: the pay the professional wants per hour worked.
    OwnerTargetHourly,
    HourlyWage,
    Commission,
    HourlyPlusCommission,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Margin,
    Markup,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Position {
    Budget,
    Standard,
    Premium,
    Luxury,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OverheadBasis {
    /// Overhead accrues for all time the chair is occupied (hands-on + processing + setup/cleanup).
    Occupied,
    /// Overhead accrues only for hands-on time (use when you work on other clients during processing).
    HandsOn,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoundMode {
    Up,
    Nearest,
    Down,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Overhead {
    pub rent: Decimal,
    pub utilities: Decimal,
    pub insurance: Decimal,
    pub software: Decimal,
    pub other: Decimal,
    pub other_label: String,
}

impl Overhead {
    pub fn total(&self) -> Decimal {
        self.rent + self.utilities + self.insurance + self.software + self.other
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProfileData {
    pub kind: ProfileKind,
    pub specialty: String,
    pub experience_level: String,
    pub location_id: Option<i64>,
    pub comp_model: CompModel,
    pub hourly_rate: Decimal,
    pub employer_burden_pct: Decimal,
    pub commission_pct: Decimal,
    pub retail_commission_pct: Decimal,
    pub weekly_hours: Decimal,
    pub weeks_per_year: Decimal,
    pub utilization_pct: Decimal,
    pub overhead: Overhead,
    pub processing_pct: Decimal,
    pub processing_fixed: Decimal,
    pub card_share_pct: Decimal,
    pub target_kind: TargetKind,
    pub target_pct: Decimal,
    pub position: Position,
    pub overhead_basis: OverheadBasis,
    pub rounding_increment: Decimal,
    pub rounding_mode: RoundMode,
    pub notes: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ProfileRates {
    pub monthly_hours: Decimal,
    pub monthly_billable_hours: Decimal,
    pub monthly_overhead: Decimal,
    pub overhead_per_billable_hour: Decimal,
    /// Pay per hour worked including employer burden (0 for commission-only).
    pub pay_per_hour_worked: Decimal,
    /// Pay recovered per billable hour (pay per hour worked ÷ utilization).
    pub labor_cost_per_billable_hour: Decimal,
    /// Effective service commission % (0 unless the model includes commission).
    pub commission_pct: Decimal,
    pub retail_commission_pct: Decimal,
    pub notes: Vec<String>,
}

const WEEKS_MAX: Decimal = Decimal::from_parts(5218, 0, 0, false, 2); // 52.18

impl ProfileData {
    pub fn validate(&self) -> AppResult<()> {
        non_negative("hourly_rate", "Hourly pay", self.hourly_rate)?;
        non_negative("employer_burden_pct", "Employer burden", self.employer_burden_pct)?;
        pct_below_100("commission_pct", "Service commission", self.commission_pct)?;
        pct_below_100("retail_commission_pct", "Retail commission", self.retail_commission_pct)?;
        if self.weekly_hours <= Decimal::ZERO || self.weekly_hours > Decimal::from(168) {
            return Err(AppError::invalid("weekly_hours", "Weekly working hours must be between 0 and 168."));
        }
        if self.weeks_per_year <= Decimal::ZERO || self.weeks_per_year > WEEKS_MAX {
            return Err(AppError::invalid("weeks_per_year", "Weeks worked per year must be between 0 and 52.18."));
        }
        pct_0_100("utilization_pct", "Billable utilization", self.utilization_pct)?;
        if self.utilization_pct.is_zero() {
            return Err(AppError::invalid(
                "utilization_pct",
                "Billable utilization is 0%, so there are no billable hours to spread overhead and pay over.",
            ));
        }
        for (f, l, v) in [
            ("overhead.rent", "Rent", self.overhead.rent),
            ("overhead.utilities", "Utilities", self.overhead.utilities),
            ("overhead.insurance", "Insurance", self.overhead.insurance),
            ("overhead.software", "Software", self.overhead.software),
            ("overhead.other", "Other overhead", self.overhead.other),
        ] {
            non_negative(f, l, v)?;
        }
        pct_below_100("processing_pct", "Card processing", self.processing_pct)?;
        non_negative("processing_fixed", "Per-transaction processing fee", self.processing_fixed)?;
        pct_0_100("card_share_pct", "Share of payments by card", self.card_share_pct)?;
        match self.target_kind {
            TargetKind::Margin => {
                pct_below_100("target_pct", "Target margin", self.target_pct)?;
            }
            TargetKind::Markup => {
                non_negative("target_pct", "Target markup", self.target_pct)?;
            }
        }
        if self.rounding_increment < Decimal::new(1, 2) {
            return Err(AppError::invalid("rounding_increment", "Rounding increment must be at least $0.01."));
        }
        Ok(())
    }

    pub fn rates(&self) -> AppResult<ProfileRates> {
        self.validate()?;
        let mut notes = Vec::new();
        let monthly_hours = self.weekly_hours * self.weeks_per_year / Decimal::from(12);
        let util = frac(self.utilization_pct);
        let billable = monthly_hours * util;
        let monthly_overhead = self.overhead.total();
        let overhead_per_billable_hour = monthly_overhead / billable;

        let wage_with_burden = self.hourly_rate * (Decimal::ONE + frac(self.employer_burden_pct));
        let (pay, commission) = match self.comp_model {
            CompModel::OwnerTargetHourly => {
                if !self.employer_burden_pct.is_zero() {
                    notes.push("Employer burden is ignored for owner compensation; it applies to employee wages only.".into());
                }
                if !self.commission_pct.is_zero() {
                    notes.push("Commission is ignored: owner compensation is already counted as an hourly cost.".into());
                }
                (self.hourly_rate, Decimal::ZERO)
            }
            CompModel::HourlyWage => {
                if !self.commission_pct.is_zero() {
                    notes.push("Commission is ignored for the hourly-wage model. Choose \"hourly plus commission\" if both are paid.".into());
                }
                (wage_with_burden, Decimal::ZERO)
            }
            CompModel::Commission => {
                if !self.hourly_rate.is_zero() {
                    notes.push("Hourly pay is ignored for the commission model, so labor is not counted twice.".into());
                }
                (Decimal::ZERO, self.commission_pct)
            }
            CompModel::HourlyPlusCommission => {
                notes.push("Both an hourly wage and a commission are counted. Make sure the hourly rate does not already include expected commission.".into());
                (wage_with_burden, self.commission_pct)
            }
        };
        let labor_per_billable = pay / util;
        if self.kind == ProfileKind::ChairRenter && self.overhead.rent.is_zero() {
            notes.push("Chair renters usually enter their chair or booth rent as monthly rent.".into());
        }
        if self.utilization_pct == HUNDRED {
            notes.push("100% utilization assumes every working minute is paid client time; most professionals are 60–85%.".into());
        }
        Ok(ProfileRates {
            monthly_hours,
            monthly_billable_hours: billable,
            monthly_overhead,
            overhead_per_billable_hour,
            pay_per_hour_worked: pay,
            labor_cost_per_billable_hour: labor_per_billable,
            commission_pct: commission,
            retail_commission_pct: self.retail_commission_pct,
            notes,
        })
    }
}

#[cfg(test)]
pub(crate) fn sample() -> ProfileData {
    use rust_decimal_macros::dec;
    ProfileData {
        kind: ProfileKind::Individual,
        specialty: "Color".into(),
        experience_level: "senior".into(),
        location_id: None,
        comp_model: CompModel::OwnerTargetHourly,
        hourly_rate: dec!(30),
        employer_burden_pct: dec!(0),
        commission_pct: dec!(0),
        retail_commission_pct: dec!(0),
        weekly_hours: dec!(40),
        weeks_per_year: dec!(48),
        utilization_pct: dec!(75),
        overhead: Overhead {
            rent: dec!(1200),
            utilities: dec!(150),
            insurance: dec!(60),
            software: dec!(40),
            other: dec!(70),
            other_label: "Laundry".into(),
        },
        processing_pct: dec!(2.9),
        processing_fixed: dec!(0.30),
        card_share_pct: dec!(100),
        target_kind: TargetKind::Margin,
        target_pct: dec!(20),
        position: Position::Standard,
        overhead_basis: OverheadBasis::Occupied,
        rounding_increment: dec!(1),
        rounding_mode: RoundMode::Up,
        notes: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn overhead_per_billable_hour() {
        let r = sample().rates().unwrap();
        // 40 h × 48 wk ÷ 12 = 160 h/month; × 75% = 120 billable h; $1,520 ÷ 120 = $12.666…
        assert_eq!(r.monthly_hours, dec!(160));
        assert_eq!(r.monthly_billable_hours, dec!(120));
        assert_eq!(r.monthly_overhead, dec!(1520));
        assert_eq!(r.overhead_per_billable_hour.round_dp(4), dec!(12.6667));
        // $30/h worked ÷ 75% = $40 per billable hour
        assert_eq!(r.labor_cost_per_billable_hour, dec!(40));
    }

    #[test]
    fn zero_billable_hours_rejected() {
        let mut p = sample();
        p.utilization_pct = dec!(0);
        assert!(matches!(p.rates(), Err(AppError::Validation { field: Some(f), .. }) if f == "utilization_pct"));
    }

    #[test]
    fn commission_model_does_not_double_count_hourly() {
        let mut p = sample();
        p.comp_model = CompModel::Commission;
        p.commission_pct = dec!(40);
        let r = p.rates().unwrap();
        assert_eq!(r.labor_cost_per_billable_hour, dec!(0));
        assert_eq!(r.commission_pct, dec!(40));
        assert!(r.notes.iter().any(|n| n.contains("not counted twice")));
    }

    #[test]
    fn wage_includes_burden_and_ignores_commission() {
        let mut p = sample();
        p.kind = ProfileKind::Employee;
        p.comp_model = CompModel::HourlyWage;
        p.hourly_rate = dec!(20);
        p.employer_burden_pct = dec!(10);
        p.commission_pct = dec!(10);
        let r = p.rates().unwrap();
        assert_eq!(r.pay_per_hour_worked, dec!(22));
        assert_eq!(r.commission_pct, dec!(0));
    }

    #[test]
    fn impossible_margin_rejected() {
        let mut p = sample();
        p.target_pct = dec!(100);
        assert!(p.validate().is_err());
        p.target_kind = TargetKind::Markup;
        assert!(p.validate().is_ok()); // a 100% markup is fine
    }
}
