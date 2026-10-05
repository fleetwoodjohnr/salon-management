//! What-if pricing: re-run a service estimate under changed assumptions and explain, line by line,
//! why the numbers moved.

use super::profiles;
use super::services::{self, estimate_with_profile, Estimate, TaxContext};
use crate::domain::costing::TimeSpec;
use crate::domain::profile::{Position, TargetKind};
use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub struct WhatIf {
    pub service_id: i64,
    pub variant_ids: Vec<i64>,
    pub target_kind: Option<TargetKind>,
    pub target_pct: Option<Decimal>,
    pub position: Option<Position>,
    /// Replace the profile's total monthly overhead (components are scaled proportionally).
    pub monthly_overhead: Option<Decimal>,
    pub utilization_pct: Option<Decimal>,
    pub time: Option<TimeSpec>,
    /// Multiply every recipe quantity.
    pub material_factor: Option<Decimal>,
    pub price: Option<Decimal>,
}

#[derive(Serialize, Clone, Debug)]
pub struct Change {
    pub item: String,
    pub before: Option<Decimal>,
    pub after: Option<Decimal>,
    pub reason: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct WhatIfResult {
    pub base: Estimate,
    pub scenario: Estimate,
    pub changes: Vec<Change>,
}

fn money(d: Decimal) -> String {
    format!("${}", d.round_dp(2))
}

fn hm(min: i64) -> String {
    if min % 60 == 0 { format!("{} h", min / 60) } else if min < 60 { format!("{min} min") } else { format!("{} h {} min", min / 60, min % 60) }
}

pub fn what_if(conn: &Connection, w: &WhatIf, tax: impl Fn(&Connection, &services::ServiceInput) -> AppResult<TaxContext>) -> AppResult<WhatIfResult> {
    let view = services::get(conn, w.service_id)?;
    let base_s = view.input.clone();
    let pid = base_s.profile_id.ok_or_else(|| AppError::msg("Give this service a work profile first."))?;
    let base_prof = profiles::get(conn, pid)?;
    let tax_ctx = tax(conn, &base_s)?;
    let base = estimate_with_profile(conn, &base_s, &w.variant_ids, tax_ctx.clone(), &base_prof)?;

    let mut s = base_s.clone();
    let mut prof = base_prof.clone();
    if let Some(k) = w.target_kind {
        s.overrides.target_kind = Some(k);
    }
    if let Some(t) = w.target_pct {
        s.overrides.target_pct = Some(t);
    }
    if let Some(p) = w.position {
        s.overrides.position = Some(p);
    }
    if let Some(t) = w.time {
        s.time = t;
    }
    if let Some(f) = w.material_factor {
        crate::domain::money::non_negative("material_factor", "Material multiplier", f)?;
        for l in s.recipe.iter_mut() {
            l.qty *= f;
        }
    }
    if let Some(p) = w.price {
        s.price = Some(p);
    }
    if let Some(u) = w.utilization_pct {
        prof.data.utilization_pct = u;
    }
    if let Some(total) = w.monthly_overhead {
        crate::domain::money::non_negative("monthly_overhead", "Monthly overhead", total)?;
        let o = &mut prof.data.overhead;
        let old = o.total();
        if old.is_zero() {
            o.other = total;
        } else {
            let f = total / old;
            o.rent *= f;
            o.utilities *= f;
            o.insurance *= f;
            o.software *= f;
            o.other *= f;
        }
    }
    prof.data.validate()?;
    let scenario = estimate_with_profile(conn, &s, &w.variant_ids, tax_ctx, &prof)?;

    let mut changes = Vec::new();
    if let (Some(a), Some(b)) = (&base.cost, &scenario.cost) {
        if a.materials != b.materials || a.waste != b.waste {
            changes.push(Change {
                item: "Materials and waste".into(),
                before: Some(a.materials + a.waste),
                after: Some(b.materials + b.waste),
                reason: match w.material_factor {
                    Some(f) => format!("Every recipe amount is multiplied by {}.", f.normalize()),
                    None => "Recipe amounts changed.".into(),
                },
            });
        }
        if a.labor != b.labor {
            let mut why = Vec::new();
            if a.time.working_min() != b.time.working_min() {
                why.push(format!("working time (hands-on, setup, cleanup) went from {} to {}", hm(a.time.working_min()), hm(b.time.working_min())));
            }
            if a.labor_rate != b.labor_rate {
                why.push(format!(
                    "pay per billable hour went from {} to {} because utilization changed from {}% to {}%",
                    money(a.labor_rate),
                    money(b.labor_rate),
                    base_prof.data.utilization_pct.normalize(),
                    prof.data.utilization_pct.normalize()
                ));
            }
            changes.push(Change { item: "Labor".into(), before: Some(a.labor), after: Some(b.labor), reason: format!("{}.", capitalize(&why.join("; and "))) });
        }
        if a.overhead != b.overhead {
            let mut why = Vec::new();
            if a.overhead_hours != b.overhead_hours {
                why.push(format!("overhead time went from {} to {}", hm(a.time.occupied_min()), hm(b.time.occupied_min())));
            }
            if a.overhead_rate != b.overhead_rate {
                let mut r = format!("overhead per billable hour went from {} to {}", money(a.overhead_rate), money(b.overhead_rate));
                if w.monthly_overhead.is_some() {
                    r += &format!(" (monthly overhead {} → {})", money(base_prof.data.overhead.total()), money(prof.data.overhead.total()));
                }
                if w.utilization_pct.is_some() && w.utilization_pct != Some(base_prof.data.utilization_pct) {
                    r += " (fewer or more billable hours to spread it over)";
                }
                why.push(r);
            }
            changes.push(Change { item: "Overhead".into(), before: Some(a.overhead), after: Some(b.overhead), reason: format!("{}.", capitalize(&why.join("; and "))) });
        }
    }
    if let (Some(pa), Some(pb)) = (&base.pricing, &scenario.pricing) {
        let (ta, tb) = (pa.targets.as_ref().map(|t| t.target_price), pb.targets.as_ref().map(|t| t.target_price));
        if ta != tb {
            let mut why = Vec::new();
            if let (Some(a), Some(b)) = (&base.cost, &scenario.cost) {
                if a.fixed_total != b.fixed_total {
                    why.push(format!("costs before fees changed by {}", money(b.fixed_total - a.fixed_total)));
                }
            }
            if pa.params.target_kind != pb.params.target_kind || pa.params.target_pct != pb.params.target_pct {
                why.push(format!(
                    "the target changed from {}% {} to {}% {}",
                    pa.params.target_pct.normalize(),
                    kind(pa.params.target_kind),
                    pb.params.target_pct.normalize(),
                    kind(pb.params.target_kind)
                ));
            }
            why.push("commission and card fees are re-solved at the new price".into());
            changes.push(Change { item: "Price needed for target".into(), before: ta, after: tb, reason: format!("{}.", capitalize(&why.join("; "))) });
        }
        let (ma, mb) = (pa.at_price.as_ref().and_then(|a| a.margin_pct), pb.at_price.as_ref().and_then(|a| a.margin_pct));
        if ma != mb {
            changes.push(Change {
                item: "Margin at your price".into(),
                before: ma,
                after: mb,
                reason: if pa.chosen_price != pb.chosen_price {
                    format!("The price changed from {} to {}.", pa.chosen_price.map(money).unwrap_or("none".into()), pb.chosen_price.map(money).unwrap_or("none".into()))
                } else {
                    "Same price, different costs.".into()
                },
            });
        }
        if pa.position != pb.position {
            changes.push(Change {
                item: "Market position".into(),
                before: None,
                after: None,
                reason: "Market position doesn't change costs. It picks which part of observed local prices to suggest (lower quarter, median, upper quarter or top tenth), shown under Market evidence.".into(),
            });
        }
    }
    Ok(WhatIfResult { base, scenario, changes })
}

fn kind(k: TargetKind) -> &'static str {
    match k {
        TargetKind::Margin => "margin",
        TargetKind::Markup => "markup",
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::inventory::save_product;
    use crate::db::inventory::tests::{buy, product_input};
    use crate::db::open_memory;
    use crate::domain::profile::sample;
    use rust_decimal_macros::dec;

    fn unknown(_: &Connection, _: &services::ServiceInput) -> AppResult<TaxContext> {
        Ok(TaxContext { taxable: None, rate_pct: None, prices_include_tax: false, label: "x".into(), resolved: false })
    }

    #[test]
    fn explains_each_change() {
        let conn = open_memory();
        let prof = profiles::save(&conn, None, "Me", &sample()).unwrap();
        let dev = save_product(&conn, &product_input("Developer", "fl_oz")).unwrap();
        buy(&conn, dev, dec!(32), "fl_oz", dec!(24));
        let mut s = services::tests::service_input(prof, dev);
        s.variants.clear();
        let id = services::save(&conn, &s).unwrap();
        let r = what_if(
            &conn,
            &WhatIf { service_id: id, utilization_pct: Some(dec!(60)), monthly_overhead: Some(dec!(1824)), target_pct: Some(dec!(30)), material_factor: Some(dec!(2)), ..Default::default() },
            unknown,
        )
        .unwrap();
        let items: Vec<&str> = r.changes.iter().map(|c| c.item.as_str()).collect();
        assert!(items.contains(&"Materials and waste"));
        assert!(items.contains(&"Labor"));
        assert!(items.contains(&"Overhead"));
        assert!(items.contains(&"Price needed for target"));
        let labor = r.changes.iter().find(|c| c.item == "Labor").unwrap();
        assert!(labor.reason.contains("utilization changed from 75% to 60%"), "{}", labor.reason);
        // $30/h ÷ 60% = $50 per billable hour, 60 working minutes → $50
        assert_eq!(labor.after, Some(dec!(50)));
        let target = r.changes.iter().find(|c| c.item == "Price needed for target").unwrap();
        assert!(target.reason.contains("20% margin to 30% margin"), "{}", target.reason);
        // the saved service and profile are untouched
        assert_eq!(profiles::get(&conn, prof).unwrap().data.utilization_pct, dec!(75));
    }
}
