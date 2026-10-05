//! Units of measure. Quantities are stored in a base unit per dimension: grams, millilitres or
//! pieces. Conversion factors are exact by definition (NIST): 1 oz (avoirdupois) = 28.349523125 g,
//! 1 lb = 453.59237 g, 1 US fl oz = 29.5735295625 mL, 1 US gal = 128 US fl oz = 3785.411784 mL.
//!
//! Weight ounces (`oz_wt`) and fluid ounces (`fl_oz`) are different units. Mass and volume convert
//! only through a product-specific density; nothing assumes the density of water.

use crate::error::{AppError, AppResult};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    Mass,
    Volume,
    Count,
}

impl Dimension {
    pub fn as_str(self) -> &'static str {
        match self {
            Dimension::Mass => "mass",
            Dimension::Volume => "volume",
            Dimension::Count => "count",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "mass" => Ok(Dimension::Mass),
            "volume" => Ok(Dimension::Volume),
            "count" => Ok(Dimension::Count),
            _ => Err(AppError::msg(format!("Unknown dimension {s}"))),
        }
    }
    pub fn base_unit(self) -> &'static str {
        match self {
            Dimension::Mass => "g",
            Dimension::Volume => "ml",
            Dimension::Count => "piece",
        }
    }
}

#[derive(Serialize, Clone, Copy, Debug)]
pub struct UnitDef {
    pub code: &'static str,
    pub label: &'static str,
    pub dimension: Dimension,
    /// How many base units (g, mL, piece) one of this unit is.
    #[serde(with = "rust_decimal::serde::str")]
    pub to_base: Decimal,
}

const fn d(num: i64, scale: u32) -> Decimal {
    Decimal::from_parts(num as u32, (num >> 32) as u32, 0, false, scale)
}

pub const UNITS: &[UnitDef] = &[
    UnitDef { code: "g", label: "grams (g)", dimension: Dimension::Mass, to_base: d(1, 0) },
    UnitDef { code: "kg", label: "kilograms (kg)", dimension: Dimension::Mass, to_base: d(1000, 0) },
    UnitDef { code: "oz_wt", label: "ounces by weight (oz)", dimension: Dimension::Mass, to_base: d(28349523125, 9) },
    UnitDef { code: "lb", label: "pounds (lb)", dimension: Dimension::Mass, to_base: d(45359237, 5) },
    UnitDef { code: "ml", label: "millilitres (mL)", dimension: Dimension::Volume, to_base: d(1, 0) },
    UnitDef { code: "l", label: "litres (L)", dimension: Dimension::Volume, to_base: d(1000, 0) },
    UnitDef { code: "fl_oz", label: "US fluid ounces (fl oz)", dimension: Dimension::Volume, to_base: d(295735295625, 10) },
    UnitDef { code: "gal", label: "US gallons (gal)", dimension: Dimension::Volume, to_base: d(3785411784, 6) },
    UnitDef { code: "piece", label: "pieces", dimension: Dimension::Count, to_base: d(1, 0) },
];

pub fn unit(code: &str) -> AppResult<&'static UnitDef> {
    UNITS
        .iter()
        .find(|u| u.code == code)
        .ok_or_else(|| AppError::msg(format!("Unknown unit \"{code}\".")))
}

/// A product-defined unit such as "pump = 2 mL" or "foil = 1 piece".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CustomUnit {
    pub name: String,
    pub qty: Decimal,
    pub unit: String,
}

/// What a specific product allows: its stock dimension, optional density and custom units.
#[derive(Clone, Debug, Default)]
pub struct ProductUnits {
    pub dimension: Option<Dimension>,
    /// grams per millilitre, entered by the user from the product's data sheet
    pub density_g_per_ml: Option<Decimal>,
    pub custom: Vec<CustomUnit>,
}

impl ProductUnits {
    /// Convert `qty` of `unit_code` (standard code or a custom unit name) to this product's base
    /// unit (g, mL or piece according to its dimension).
    pub fn to_base(&self, qty: Decimal, unit_code: &str) -> AppResult<Decimal> {
        let dim = self.dimension.ok_or_else(|| AppError::msg("Product has no stock unit."))?;
        let (q, from) = self.resolve(qty, unit_code)?;
        convert_dims(q * from.to_base, from.dimension, dim, self.density_g_per_ml, unit_code)
    }

    /// Convert a quantity in this product's base unit into `unit_code`.
    pub fn from_base(&self, base_qty: Decimal, unit_code: &str) -> AppResult<Decimal> {
        let one = self.to_base(Decimal::ONE, unit_code)?;
        if one.is_zero() {
            return Err(AppError::msg(format!("Unit \"{unit_code}\" has zero size.")));
        }
        Ok(base_qty / one)
    }

    /// Expand a custom unit into (qty, standard unit). Custom units nest at most one level.
    fn resolve(&self, qty: Decimal, unit_code: &str) -> AppResult<(Decimal, &'static UnitDef)> {
        if let Ok(u) = unit(unit_code) {
            return Ok((qty, u));
        }
        let c = self
            .custom
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(unit_code))
            .ok_or_else(|| AppError::msg(format!("Unknown unit \"{unit_code}\" for this product. Define it under the product's custom units first.")))?;
        Ok((qty * c.qty, unit(&c.unit)?))
    }
}

/// Convert an amount already in the base unit of `from` into the base unit of `to`.
fn convert_dims(v: Decimal, from: Dimension, to: Dimension, density: Option<Decimal>, unit_code: &str) -> AppResult<Decimal> {
    use Dimension::*;
    match (from, to) {
        (a, b) if a == b => Ok(v),
        (Mass, Volume) | (Volume, Mass) => {
            let rho = density.filter(|r| *r > Decimal::ZERO).ok_or_else(|| {
                AppError::msg(format!(
                    "Can't convert {unit_code} to {}: this product has no density. Add its density (g per mL) from the product data sheet, or record usage in a {} unit.",
                    if to == Volume { "volume" } else { "weight" },
                    to.as_str()
                ))
            })?;
            Ok(if from == Mass { v / rho } else { v * rho })
        }
        _ => Err(AppError::msg(format!(
            "{unit_code} measures {} but this product is counted by {}. These can't be converted; define a custom unit (e.g. \"1 tube = 60 g\") instead.",
            from.as_str(),
            to.as_str()
        ))),
    }
}

/// Validate a custom unit definition against the product's dimension.
pub fn validate_custom(pu: &ProductUnits, c: &CustomUnit) -> AppResult<()> {
    let name = c.name.trim();
    if name.is_empty() {
        return Err(AppError::invalid("name", "Name the unit, e.g. pump or scoop."));
    }
    if unit(name).is_ok() {
        return Err(AppError::invalid("name", "That name is already a standard unit."));
    }
    if c.qty <= Decimal::ZERO {
        return Err(AppError::invalid("qty", "A custom unit must be more than zero."));
    }
    if unit(&c.unit).is_err() {
        return Err(AppError::invalid("unit", "Define custom units in terms of a standard unit."));
    }
    let probe = ProductUnits { dimension: pu.dimension, density_g_per_ml: pu.density_g_per_ml, custom: vec![] };
    probe.to_base(c.qty, &c.unit).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn vol() -> ProductUnits {
        ProductUnits { dimension: Some(Dimension::Volume), ..Default::default() }
    }

    #[test]
    fn factors_are_exact() {
        assert_eq!(unit("oz_wt").unwrap().to_base, dec!(28.349523125));
        assert_eq!(unit("lb").unwrap().to_base, dec!(453.59237));
        assert_eq!(unit("fl_oz").unwrap().to_base, dec!(29.5735295625));
        assert_eq!(unit("gal").unwrap().to_base, dec!(3785.411784));
        // 1 gal = 128 fl oz exactly; 1 lb = 16 oz exactly
        assert_eq!(unit("fl_oz").unwrap().to_base * dec!(128), unit("gal").unwrap().to_base);
        assert_eq!(unit("oz_wt").unwrap().to_base * dec!(16), unit("lb").unwrap().to_base);
    }

    #[test]
    fn compatible_conversions() {
        let p = vol();
        assert_eq!(p.to_base(dec!(32), "fl_oz").unwrap(), dec!(946.352946));
        assert_eq!(p.from_base(dec!(946.352946), "fl_oz").unwrap(), dec!(32));
        assert_eq!(p.to_base(dec!(1.5), "l").unwrap(), dec!(1500));
        assert_eq!(p.from_base(dec!(3785.411784), "fl_oz").unwrap(), dec!(128));
    }

    #[test]
    fn weight_ounce_is_not_fluid_ounce() {
        let p = vol();
        let err = p.to_base(dec!(2), "oz_wt").unwrap_err().to_string();
        assert!(err.contains("density"), "{err}");
    }

    #[test]
    fn mass_volume_needs_density_and_uses_it() {
        let mut p = ProductUnits { dimension: Some(Dimension::Mass), ..Default::default() };
        assert!(p.to_base(dec!(10), "ml").is_err());
        p.density_g_per_ml = Some(dec!(1.25)); // a cream heavier than water
        assert_eq!(p.to_base(dec!(10), "ml").unwrap(), dec!(12.5));
        assert_eq!(p.from_base(dec!(12.5), "ml").unwrap(), dec!(10));
    }

    #[test]
    fn count_never_converts_to_mass() {
        let p = ProductUnits { dimension: Some(Dimension::Count), ..Default::default() };
        assert!(p.to_base(dec!(1), "g").is_err());
        assert_eq!(p.to_base(dec!(3), "piece").unwrap(), dec!(3));
    }

    #[test]
    fn custom_units() {
        let mut p = vol();
        p.custom.push(CustomUnit { name: "pump".into(), qty: dec!(2), unit: "ml".into() });
        p.custom.push(CustomUnit { name: "application".into(), qty: dec!(0.5), unit: "fl_oz".into() });
        assert_eq!(p.to_base(dec!(3), "pump").unwrap(), dec!(6));
        assert_eq!(p.to_base(dec!(2), "Application").unwrap(), dec!(29.5735295625));
        assert!(p.to_base(dec!(1), "scoop").is_err());
        assert!(validate_custom(&p, &CustomUnit { name: "scoop".into(), qty: dec!(5), unit: "g".into() }).is_err());
        assert!(validate_custom(&p, &CustomUnit { name: "ml".into(), qty: dec!(5), unit: "ml".into() }).is_err());
        assert!(validate_custom(&p, &CustomUnit { name: "squirt".into(), qty: dec!(0), unit: "ml".into() }).is_err());
    }

    #[test]
    fn fractional_quantities_stay_exact() {
        let p = vol();
        assert_eq!(p.to_base(dec!(0.125), "fl_oz").unwrap(), dec!(3.6966911953125));
    }
}
