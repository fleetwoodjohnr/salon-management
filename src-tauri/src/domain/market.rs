//! Observed-price statistics with transparent comparable-selection rules.
//!
//! Rules, in order (each excluded observation records why):
//! 1. Recency: observed within `max_age_days` of the reference date.
//! 2. Distance: within `radius_km` when both points have coordinates (unknown distance is kept and
//!    counted as "distance unknown").
//! 3. Variant match: when the target specifies hair length or stylist level, observations with a
//!    different known value are excluded (unspecified values are kept and flagged). An observation
//!    whose listed duration differs from yours by more than 50% is excluded.
//! 4. "Starting at" prices are lower bounds, not prices: excluded from statistics unless allowed.
//! 5. Duplicates: the same business and price within 30 days counts once (newest kept).
//! 6. Outliers: with 5+ prices, values outside 1.5 × IQR of the quartiles are excluded.
//!
//! Quantiles use linear interpolation between order statistics (the common "type 7" definition).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Obs {
    pub id: i64,
    pub competitor_id: i64,
    pub price: Decimal,
    pub price_type: String,
    pub observed_on: String,
    pub distance_km: Option<f64>,
    pub hair_length: String,
    pub stylist_level: String,
    pub duration_min: Option<i64>,
    pub excluded_by_user: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Rules {
    pub reference_date: String,
    pub max_age_days: i64,
    pub radius_km: Option<f64>,
    pub hair_length: String,
    pub stylist_level: String,
    pub include_starting_at: bool,
    /// Your service's chair time; observations listing a duration more than 50% different are
    /// not comparable.
    pub target_minutes: Option<i64>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Decision {
    pub id: i64,
    pub included: bool,
    pub reason: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Stats {
    pub n: usize,
    pub businesses: usize,
    pub min: Decimal,
    pub q1: Decimal,
    pub median: Decimal,
    pub q3: Decimal,
    pub p90: Decimal,
    pub max: Decimal,
    pub median_age_days: i64,
    pub newest: String,
    pub oldest: String,
    pub with_distance: usize,
    pub max_distance_km: Option<f64>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Evidence {
    /// "none" | "low" | "medium" | "high"
    pub quality: String,
    pub reasons: Vec<String>,
    pub stats: Option<Stats>,
    pub decisions: Vec<Decision>,
    pub starting_at_count: usize,
}

fn days_between(a: &str, b: &str) -> Option<i64> {
    let a = chrono::NaiveDate::parse_from_str(a, "%Y-%m-%d").ok()?;
    let b = chrono::NaiveDate::parse_from_str(b, "%Y-%m-%d").ok()?;
    Some((b - a).num_days())
}

/// Type-7 quantile of sorted values.
pub fn quantile(sorted: &[Decimal], p: Decimal) -> Decimal {
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    let h = Decimal::from(n as i64 - 1) * p;
    let lo = h.floor();
    let i = lo.to_string().parse::<usize>().unwrap_or(0);
    let frac = h - lo;
    if i + 1 >= n {
        return sorted[n - 1];
    }
    sorted[i] + (sorted[i + 1] - sorted[i]) * frac
}

pub fn evaluate(obs: &[Obs], r: &Rules) -> Evidence {
    let mut decisions = Vec::new();
    let mut kept: Vec<&Obs> = Vec::new();
    let mut starting_at = 0;
    for o in obs {
        let reason = if o.excluded_by_user {
            Some("Excluded by you".to_string())
        } else if days_between(&o.observed_on, &r.reference_date).is_none_or(|d| d > r.max_age_days || d < 0) {
            Some(format!("Older than {} days", r.max_age_days))
        } else if let (Some(max), Some(d)) = (r.radius_km, o.distance_km) {
            (d > max).then(|| format!("{d:.1} km away (limit {max} km)"))
        } else {
            None
        }
        .or_else(|| (!r.hair_length.is_empty() && !o.hair_length.is_empty() && o.hair_length != r.hair_length).then(|| format!("Different hair length ({})", o.hair_length)))
        .or_else(|| (!r.stylist_level.is_empty() && !o.stylist_level.is_empty() && o.stylist_level != r.stylist_level).then(|| format!("Different stylist level ({})", o.stylist_level)))
        .or_else(|| match (r.target_minutes, o.duration_min) {
            (Some(t), Some(d)) if t > 0 && (d - t).abs() * 2 > t => Some(format!("Listed as {d} min; yours is {t} min")),
            _ => None,
        })
        .or_else(|| {
            if o.price_type == "starting_at" {
                starting_at += 1;
                (!r.include_starting_at).then(|| "\"Starting at\" price: a lower bound, not a price".to_string())
            } else {
                None
            }
        });
        if reason.is_none() {
            kept.push(o);
        }
        decisions.push(Decision { id: o.id, included: reason.is_none(), reason });
    }
    // Duplicates: same business + price within 30 days → keep the newest.
    kept.sort_by(|a, b| b.observed_on.cmp(&a.observed_on));
    let mut unique: Vec<&Obs> = Vec::new();
    for o in kept {
        if let Some(d) = unique.iter().find(|u| u.competitor_id == o.competitor_id && u.price == o.price && days_between(&o.observed_on, &u.observed_on).is_some_and(|x| x.abs() <= 30)) {
            let keep = d.id;
            set(&mut decisions, o.id, format!("Duplicate of observation {keep}"));
        } else {
            unique.push(o);
        }
    }
    // Outliers.
    let mut prices: Vec<Decimal> = unique.iter().map(|o| o.price).collect();
    prices.sort();
    if prices.len() >= 5 {
        let q1 = quantile(&prices, Decimal::new(25, 2));
        let q3 = quantile(&prices, Decimal::new(75, 2));
        let fence = (q3 - q1) * Decimal::new(15, 1);
        let (lo, hi) = (q1 - fence, q3 + fence);
        unique.retain(|o| {
            let out = o.price < lo || o.price > hi;
            if out {
                set(&mut decisions, o.id, format!("Outlier (outside ${} to ${})", lo.round_dp(2), hi.round_dp(2)));
            }
            !out
        });
        prices = unique.iter().map(|o| o.price).collect();
        prices.sort();
    }
    let mut reasons = Vec::new();
    if prices.is_empty() {
        if !obs.is_empty() {
            reasons.push(format!("None of the {} recorded prices meet the comparison rules (see the reasons beside each).", obs.len()));
        }
        return Evidence { quality: "none".into(), reasons, stats: None, decisions, starting_at_count: starting_at };
    }
    let mut ages: Vec<i64> = unique.iter().filter_map(|o| days_between(&o.observed_on, &r.reference_date)).collect();
    ages.sort();
    let businesses = {
        let mut b: Vec<i64> = unique.iter().map(|o| o.competitor_id).collect();
        b.sort();
        b.dedup();
        b.len()
    };
    let dists: Vec<f64> = unique.iter().filter_map(|o| o.distance_km).collect();
    let stats = Stats {
        n: prices.len(),
        businesses,
        min: prices[0],
        q1: quantile(&prices, Decimal::new(25, 2)),
        median: quantile(&prices, Decimal::new(5, 1)),
        q3: quantile(&prices, Decimal::new(75, 2)),
        p90: quantile(&prices, Decimal::new(9, 1)),
        max: *prices.last().unwrap(),
        median_age_days: ages[ages.len() / 2],
        newest: unique.iter().map(|o| o.observed_on.clone()).max().unwrap(),
        oldest: unique.iter().map(|o| o.observed_on.clone()).min().unwrap(),
        with_distance: dists.len(),
        max_distance_km: dists.iter().cloned().fold(None, |m: Option<f64>, d| Some(m.map_or(d, |m| m.max(d)))),
    };
    let quality = if stats.n >= 8 && stats.businesses >= 5 && stats.median_age_days <= 180 {
        "high"
    } else if stats.n >= 4 && stats.businesses >= 3 && stats.median_age_days <= 365 {
        "medium"
    } else {
        "low"
    };
    reasons.push(format!("{} price{} from {} business{}.", stats.n, if stats.n == 1 { "" } else { "s" }, stats.businesses, if stats.businesses == 1 { "" } else { "es" }));
    reasons.push(format!("Typical observation is {} days old.", stats.median_age_days));
    if stats.with_distance < stats.n {
        reasons.push(format!("{} of {} have no location, so distance couldn't be checked.", stats.n - stats.with_distance, stats.n));
    }
    if quality != "high" {
        reasons.push(match quality {
            "medium" => "High quality needs 8+ prices from 5+ businesses, mostly from the last 6 months.".into(),
            _ => "Medium quality needs 4+ prices from 3+ businesses, mostly from the last year.".into(),
        });
    }
    if r.include_starting_at && starting_at > 0 {
        reasons.push("Includes \"starting at\" prices, which understate typical prices.".into());
    }
    Evidence { quality: quality.into(), reasons, stats: Some(stats), decisions, starting_at_count: starting_at }
}

fn set(decisions: &mut [Decision], id: i64, reason: String) {
    if let Some(d) = decisions.iter_mut().find(|d| d.id == id) {
        d.included = false;
        d.reason = Some(reason);
    }
}

/// Great-circle distance in km (display/filter only; not money).
pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dp = (lat2 - lat1).to_radians();
    let dl = (lon2 - lon1).to_radians();
    let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    6371.0 * 2.0 * a.sqrt().asin()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn o(id: i64, comp: i64, price: Decimal, date: &str) -> Obs {
        Obs { id, competitor_id: comp, price, price_type: "exact".into(), observed_on: date.into(), distance_km: Some(2.0), hair_length: String::new(), stylist_level: String::new(), duration_min: None, excluded_by_user: false }
    }
    fn rules() -> Rules {
        Rules { reference_date: "2026-10-01".into(), max_age_days: 540, radius_km: Some(15.0), hair_length: String::new(), stylist_level: String::new(), include_starting_at: false, target_minutes: None }
    }

    #[test]
    fn quantiles() {
        let v = [dec!(10), dec!(20), dec!(30), dec!(40)];
        assert_eq!(quantile(&v, dec!(0.5)), dec!(25));
        assert_eq!(quantile(&v, dec!(0.25)), dec!(17.5));
        assert_eq!(quantile(&[dec!(7)], dec!(0.9)), dec!(7));
    }

    #[test]
    fn insufficient_and_rules() {
        assert_eq!(evaluate(&[], &rules()).quality, "none");
        let mut old = o(1, 1, dec!(80), "2024-01-01");
        old.distance_km = Some(1.0);
        let mut far = o(2, 2, dec!(80), "2026-09-01");
        far.distance_km = Some(40.0);
        let mut starting = o(3, 3, dec!(60), "2026-09-01");
        starting.price_type = "starting_at".into();
        let e = evaluate(&[old, far, starting], &rules());
        assert_eq!(e.quality, "none");
        assert!(e.decisions.iter().all(|d| !d.included));
        assert!(e.decisions[2].reason.as_ref().unwrap().contains("lower bound"));
    }

    #[test]
    fn duplicates_outliers_and_quality() {
        let mut obs = vec![
            o(1, 1, dec!(80), "2026-09-01"),
            o(2, 1, dec!(80), "2026-09-20"), // duplicate of the newer one
            o(3, 2, dec!(85), "2026-08-15"),
            o(4, 3, dec!(90), "2026-08-01"),
            o(5, 4, dec!(95), "2026-07-01"),
            o(6, 5, dec!(100), "2026-06-01"),
            o(7, 6, dec!(400), "2026-09-10"), // outlier
            o(8, 7, dec!(88), "2026-09-05"),
            o(9, 8, dec!(92), "2026-09-06"),
        ];
        obs[0].hair_length = "long".into();
        let e = evaluate(&obs, &rules());
        let s = e.stats.unwrap();
        assert_eq!(s.n, 7);
        assert!(e.decisions.iter().find(|d| d.id == 7).unwrap().reason.as_ref().unwrap().starts_with("Outlier"));
        assert!(e.decisions.iter().find(|d| d.id == 1).unwrap().reason.as_ref().unwrap().starts_with("Duplicate"));
        assert_eq!(s.median, dec!(90));
        assert_eq!(e.quality, "medium"); // 7 prices < 8 needed for high
        // hair length rule
        let mut r = rules();
        r.hair_length = "short".into();
        let e2 = evaluate(&obs, &r);
        assert!(e2.decisions.iter().find(|d| d.id == 1).unwrap().reason.as_ref().unwrap().contains("hair length"));
        // duration rule: a 30-minute listing isn't comparable to a 95-minute service
        let mut r3 = rules();
        r3.target_minutes = Some(95);
        let mut quick = obs.clone();
        quick[2].duration_min = Some(30);
        quick[3].duration_min = Some(90);
        let e3 = evaluate(&quick, &r3);
        assert!(e3.decisions.iter().find(|d| d.id == 3).unwrap().reason.as_ref().unwrap().contains("30 min"));
        assert!(e3.decisions.iter().find(|d| d.id == 4).unwrap().included);
    }

    #[test]
    fn distance() {
        let d = haversine_km(46.992, -122.9168, 47.6097, -122.3331);
        assert!((d - 83.0).abs() < 3.0, "{d}");
    }
}
