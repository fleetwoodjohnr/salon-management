//! Sample data for the separate demo workspace (and the large-dataset test). Everything is
//! fictional: business names, people, addresses and the tax rate are invented and labelled.
//! Data goes through the same functions the app uses, so it obeys every rule real data does.

use crate::db::{appointments, clients, core, expenses, inventory, market, profiles, sales, services, tax};
use crate::domain::costing::TimeSpec;
use crate::domain::profile::{CompModel, OverheadBasis, Position, ProfileData, ProfileKind, RoundMode, TargetKind};
use crate::domain::sale::LineKind;
use crate::error::AppResult;
use chrono::{Datelike, NaiveDate};
use rusqlite::Connection;
use rust_decimal::Decimal;

/// Small deterministic generator so the demo is identical every time.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn chance(&mut self, pct: u64) -> bool {
        self.below(100) < pct
    }
}

fn d(s: &str) -> Decimal {
    s.parse().unwrap()
}

fn profile(kind: ProfileKind, comp: CompModel, rate: &str, commission: &str, rent: &str) -> ProfileData {
    ProfileData {
        kind,
        specialty: String::new(),
        experience_level: "experienced".into(),
        location_id: None,
        comp_model: comp,
        hourly_rate: d(rate),
        employer_burden_pct: d(if kind == ProfileKind::Employee { "10" } else { "0" }),
        commission_pct: d(commission),
        retail_commission_pct: d("10"),
        weekly_hours: d("36"),
        weeks_per_year: d("48"),
        utilization_pct: d("75"),
        overhead: crate::domain::profile::Overhead { rent: d(rent), utilities: d("180"), insurance: d("65"), software: d("45"), other: d("90"), other_label: "Laundry and cleaning".into() },
        processing_pct: d("2.9"),
        processing_fixed: d("0.30"),
        card_share_pct: d("85"),
        target_kind: TargetKind::Margin,
        target_pct: d("20"),
        position: Position::Standard,
        overhead_basis: OverheadBasis::Occupied,
        rounding_increment: d("5"),
        rounding_mode: RoundMode::Up,
        notes: String::new(),
    }
}

/// Populate an empty workspace. `months` of history ending at `today`; `daily_sales` per open day.
pub fn seed(conn: &Connection, today: NaiveDate, months: u32, daily_sales: u32) -> AppResult<()> {
    let mut rng = Rng(20261004);
    let start = today - chrono::Duration::days(months as i64 * 30);
    let ds = |x: NaiveDate| x.format("%Y-%m-%d").to_string();

    let loc = core::save_location(
        conn,
        &core::Location {
            id: None,
            name: "Demo studio".into(),
            address_line: "100 Example Street".into(),
            city: "Springfield".into(),
            state: "WA".into(),
            postal_code: "98000".into(),
            latitude: Some("47.0000".into()),
            longitude: Some("-122.9000".into()),
            geo_precision: Some("manual".into()),
            geo_source: Some("Demo data (fictional location)".into()),
            state_fips: None,
            county_fips: None,
            place_fips: None,
            archived: false,
        },
    )?;
    let mut biz = core::Business { name: "Demo Salon (sample data)".into(), primary_location_id: Some(loc), onboarding_completed: true, ..Default::default() };
    biz.schedule = (0..7).map(|dd| core::DaySchedule { day: dd, open: dd < 6, start: "09:00".into(), end: "18:00".into() }).collect();
    core::save_business(conn, &biz)?;
    tax::save_rate(
        conn,
        &tax::RateSetInput {
            location_id: loc,
            state_rate: None,
            county_rate: None,
            city_rate: None,
            district_rate: None,
            total_rate: Some(d("8.5")),
            jurisdiction_label: "Demo rate (fictional)".into(),
            jurisdiction_code: None,
            source: "manual".into(),
            source_url: None,
            precision: "address".into(),
            status: "manual".into(),
            effective_date: None,
            dataset_period: None,
            retrieved_at: None,
            note: "Invented for the demo. Not a real tax rate.".into(),
        },
    )?;
    tax::set_rule(conn, loc, "service", "taxable", "Demo assumption (not tax advice)")?;
    tax::set_rule(conn, loc, "retail", "taxable", "Demo assumption (not tax advice)")?;
    tax::set_rule(conn, loc, "tips", "exempt", "Demo assumption (not tax advice)")?;

    let owner = profiles::save(conn, None, "Owner – color", &profile(ProfileKind::Individual, CompModel::OwnerTargetHourly, "38", "0", "1400"))?;
    let stylist = profiles::save(conn, None, "Stylist – commission", &profile(ProfileKind::Employee, CompModel::Commission, "0", "40", "700"))?;
    let ana = core::save_staff(conn, &core::Staff { id: None, name: "Avery (owner)".into(), color: "grape".into(), default_profile_id: Some(owner), location_id: Some(loc), archived: false })?;
    let ben = core::save_staff(conn, &core::Staff { id: None, name: "Jordan".into(), color: "teal".into(), default_profile_id: Some(stylist), location_id: Some(loc), archived: false })?;
    let staff = [ana, ben];

    let sup = inventory::save_supplier(conn, &inventory::Supplier { id: None, name: "Example Beauty Supply".into(), contact: String::new(), phone: String::new(), email: String::new(), website: String::new(), notes: "Fictional supplier".into(), archived: false })?;
    let back = inventory::save_storage(conn, &inventory::StorageLocation { id: None, name: "Back room".into(), location_id: Some(loc), archived: false })?;
    let product = |name: &str, cat: &str, unit: &str, rp: Option<&str>, reorder: &str| inventory::ProductInput {
        id: None,
        name: name.into(),
        brand: "Demo Pro".into(),
        category: cat.into(),
        subcategory: String::new(),
        sku: String::new(),
        barcode: String::new(),
        supplier_id: Some(sup),
        stock_unit: unit.into(),
        density_g_per_ml: None,
        default_storage_id: Some(back),
        reorder_point: Some(d(reorder)),
        reorder_qty: None,
        retail_price: rp.map(d),
        notes: String::new(),
        custom_units: vec![],
    };
    let mut color = product("Permanent color 7N", "professional", "g", None, "300");
    color.custom_units.push(crate::domain::units::CustomUnit { name: "tube".into(), qty: d("60"), unit: "g".into() });
    let color = inventory::save_product(conn, &color)?;
    let dev = inventory::save_product(conn, &product("Developer 20 vol", "professional", "fl_oz", None, "64"))?;
    let lightener = inventory::save_product(conn, &product("Lightener", "professional", "oz_wt", None, "16"))?;
    let gloss = inventory::save_product(conn, &product("Gloss toner", "professional", "ml", None, "500"))?;
    let foil = inventory::save_product(conn, &product("Foil sheets", "consumable", "piece", None, "200"))?;
    let gloves = inventory::save_product(conn, &product("Nitrile gloves", "consumable", "piece", None, "100"))?;
    let shampoo = inventory::save_product(conn, &product("Repair shampoo 300 ml", "retail", "piece", Some("26"), "6"))?;
    let oil = inventory::save_product(conn, &product("Finishing oil 50 ml", "retail", "piece", Some("32"), "4"))?;
    // (product, packages, contents, unit, price)
    // Monthly order sized to roughly a month's use at 3 sales a day (scaled for bigger datasets).
    let restock: [(i64, &str, &str, &str, &str); 8] = [
        (color, "18", "1", "tube", "162"),
        (dev, "3", "32", "fl_oz", "66"),
        (lightener, "1", "24", "oz_wt", "44"),
        (gloss, "1", "1", "l", "30"),
        (foil, "1", "500", "piece", "18"),
        (gloves, "2", "100", "piece", "20"),
        (shampoo, "6", "1", "piece", "66"),
        (oil, "5", "1", "piece", "75"),
    ];
    let buy = |date: NaiveDate, scale: u32| -> AppResult<()> {
        let lines = restock
            .iter()
            .map(|(pid, pk, c, u, price)| inventory::PurchaseLineInput {
                product_id: *pid,
                package_count: d(pk) * Decimal::from(scale),
                contents_per_package: d(c),
                unit: u.to_string(),
                line_price: d(price) * Decimal::from(scale),
                storage_id: Some(back),
                lot_code: format!("L{}", date.format("%y%m")),
                expires_on: Some(ds(date + chrono::Duration::days(540))),
            })
            .collect();
        inventory::receive(
            conn,
            &inventory::PurchaseInput {
                kind: "purchase".into(),
                supplier_id: Some(sup),
                purchase_date: ds(date),
                invoice_ref: format!("DEMO-{}", date.format("%Y%m%d")),
                discount: d("10"),
                shipping: d("12.50"),
                nonrecoverable_tax: d("0"),
                attachment_id: None,
                notes: String::new(),
                lines,
            },
        )?;
        Ok(())
    };

    let svc = |name: &str, cat: &str, prof: i64, t: [i64; 4], price: &str, recipe: Vec<services::RecipeLineInput>, variants: Vec<services::VariantInput>| services::ServiceInput {
        id: None,
        name: name.into(),
        category: cat.into(),
        description: String::new(),
        profile_id: Some(prof),
        time: TimeSpec { hands_on_min: t[0], processing_min: t[1], setup_min: t[2], cleanup_min: t[3] },
        is_addon: false,
        waste_pct: d("8"),
        other_direct_cost: d("1.25"),
        other_direct_note: "Towels and cape laundry".into(),
        price: Some(d(price)),
        price_note: String::new(),
        overrides: Default::default(),
        tax_category: "service".into(),
        variants,
        recipe,
        recipe_note: "Demo recipe".into(),
    };
    let rl = |p: i64, q: &str, u: &str| services::RecipeLineInput { product_id: p, qty: d(q), unit: u.into(), note: String::new() };
    let long = || vec![services::VariantInput { id: None, group_name: "Hair length".into(), name: "Long".into(), hands_on_delta: 15, processing_delta: 0, material_factor: d("1.5"), price_delta: d("20") }];
    let cut = services::save(conn, &svc("Haircut and style", "Cuts", stylist, [45, 0, 5, 10], "65", vec![rl(gloves, "2", "piece")], long()))?;
    let root = services::save(conn, &svc("Root touch-up", "Color", owner, [45, 35, 5, 10], "95", vec![rl(color, "1", "tube"), rl(dev, "2", "fl_oz"), rl(gloves, "2", "piece")], vec![]))?;
    let hl = services::save(conn, &svc("Partial highlights", "Color", owner, [90, 40, 10, 10], "165", vec![rl(lightener, "2", "oz_wt"), rl(dev, "4", "fl_oz"), rl(foil, "40", "piece"), rl(gloves, "2", "piece")], long()))?;
    let glossing = services::save(conn, &svc("Gloss treatment", "Color", stylist, [20, 20, 5, 5], "45", vec![rl(gloss, "60", "ml"), rl(gloves, "2", "piece")], vec![]))?;
    let blowout = services::save(conn, &svc("Blowout", "Styling", stylist, [35, 0, 5, 5], "50", vec![], vec![]))?;
    let menu = [(cut, 35u64), (root, 20), (hl, 12), (glossing, 15), (blowout, 18)];

    let first = ["Mia", "Noah", "Ava", "Liam", "Emma", "Lucas", "Zoe", "Ethan", "Ivy", "Leo", "Nora", "Owen", "Ruby", "Eli", "Maya", "Kai", "Lena", "Theo", "Aria", "Finn"];
    let last = ["Sample", "Example", "Demo", "Placeholder", "Testwell"];
    let mut client_ids = Vec::new();
    for (i, f) in first.iter().enumerate() {
        let id = clients::save(conn, &clients::ClientInput { id: None, first_name: f.to_string(), last_name: last[i % last.len()].into(), phone: format!("555-01{i:02}"), email: String::new(), sensitivities: if i % 7 == 0 { "Sensitive scalp; patch test before color".into() } else { String::new() }, notes: "Fictional demo client".into() })?;
        client_ids.push(id);
    }
    clients::save_formula(conn, &clients::FormulaInput { formula_id: None, client_id: client_ids[0], title: "Root color".into(), service_id: Some(root), body: "7N 60 g + 20 vol 60 ml, 35 min".into(), lines: vec![], note: "Demo".into(), sale_id: None })?;

    // History
    let scale = daily_sales.div_ceil(3).max(1);
    buy(start - chrono::Duration::days(3), scale + 1)?;
    let mut day = start;
    let mut month_seen = None;
    while day < today {
        if month_seen != Some((day.year(), day.month())) {
            month_seen = Some((day.year(), day.month()));
            for (cat, amount, vendor) in [("rent", "2100", "Example Property Co."), ("utilities", "240", "City Utilities"), ("software", "45", "Booking software"), ("insurance", "130", "Example Mutual")] {
                expenses::save(conn, &expenses::ExpenseInput { id: None, expense_date: ds(day), category: cat.into(), vendor: vendor.into(), description: "Demo expense".into(), amount: d(amount), payment_method: "transfer".into(), location_id: Some(loc), attachment_id: None })?;
            }
            if day > start {
                buy(day, scale)?;
            }
        }
        if day.weekday().num_days_from_monday() < 6 {
            for n in 0..daily_sales {
                let mut pick = rng.below(100);
                let sid = menu.iter().find(|(_, w)| if pick < *w { true } else { pick -= w; false }).map(|(s, _)| *s).unwrap_or(cut);
                let s = services::get(conn, sid)?;
                let variant: Vec<i64> = if !s.input.variants.is_empty() && rng.chance(30) { vec![s.input.variants[0].id.unwrap()] } else { vec![] };
                let (time, factor, delta, names) = services::apply_variants(&s.input, &variant)?;
                let usage = services::material_lines(conn, &s.input.recipe, factor)?
                    .into_iter()
                    .map(|m| {
                        // actual usage varies ±15% around the recipe
                        let adj = Decimal::from(85 + rng.below(31) as i64) / Decimal::from(100);
                        // whole pieces for counted items
                        let dp = if m.base_unit == "piece" { 0 } else { 2 };
                        sales::UsageInput { product_id: m.product_id, planned_qty: m.qty, qty: (m.qty * adj).round_dp(dp), unit: m.unit }
                    })
                    .collect();
                let mut lines = vec![sales::SaleLineInput {
                    kind: LineKind::Service,
                    service_id: Some(sid),
                    product_id: None,
                    staff_id: None,
                    description: if names.is_empty() { s.input.name.clone() } else { format!("{} ({})", s.input.name, names.join(", ")) },
                    variant_ids: variant,
                    qty: Decimal::ONE,
                    unit_price: s.input.price.unwrap() + delta,
                    line_discount: Decimal::ZERO,
                    planned_minutes: Some(time.occupied_min()),
                    actual_minutes: if rng.chance(40) { Some(time.occupied_min() + rng.below(21) as i64 - 10) } else { None },
                    usage,
                }];
                if rng.chance(18) {
                    let (pid, price) = if rng.chance(50) { (shampoo, "26") } else { (oil, "32") };
                    lines.push(sales::SaleLineInput { kind: LineKind::Retail, service_id: None, product_id: Some(pid), staff_id: None, description: "Retail".into(), variant_ids: vec![], qty: Decimal::ONE, unit_price: d(price), line_discount: Decimal::ZERO, planned_minutes: None, actual_minutes: None, usage: vec![] });
                }
                if rng.chance(70) {
                    lines.push(sales::SaleLineInput { kind: LineKind::Tip, service_id: None, product_id: None, staff_id: None, description: "Tip".into(), variant_ids: vec![], qty: Decimal::ONE, unit_price: Decimal::from(5 + rng.below(4) as i64 * 5), line_discount: Decimal::ZERO, planned_minutes: None, actual_minutes: None, usage: vec![] });
                }
                let id = sales::save_draft(
                    conn,
                    &sales::SaleDraft {
                        id: None,
                        client_id: Some(client_ids[rng.below(client_ids.len() as u64) as usize]),
                        staff_id: Some(staff[(n as usize + rng.below(2) as usize) % 2]),
                        location_id: Some(loc),
                        appointment_id: None,
                        sale_date: ds(day),
                        sale_discount: if rng.chance(8) { d("10") } else { Decimal::ZERO },
                        notes: String::new(),
                        lines,
                    },
                )?;
                let v = sales::finalize(conn, id)?;
                let method = if rng.chance(85) { "card" } else { "cash" };
                sales::add_payment(conn, id, &sales::PaymentInput { method: method.into(), amount: v.totals.total, reference: String::new(), paid_on: ds(day) })?;
                if rng.chance(2) {
                    sales::refund(
                        conn,
                        id,
                        &sales::RefundInput { refund_date: ds(day + chrono::Duration::days(2)), reason: "Client unhappy with result (demo)".into(), method: method.into(), lines: vec![sales::RefundLineInput { sale_line_id: v.lines[0].id, qty: Decimal::ONE, restock: false }] },
                    )?;
                }
            }
        }
        if rng.chance(10) {
            inventory::record_movement(
                conn,
                &inventory::MovementInput { product_id: gloss, kind: "waste".into(), qty: d("30"), unit: "ml".into(), storage_id: Some(back), to_storage_id: None, unit_cost: None, occurred_on: ds(day), note: "Mixed too much (demo)".into() },
            )?;
        }
        day += chrono::Duration::days(1);
    }

    // Upcoming appointments for the next week
    for i in 0..12u32 {
        let date = today + chrono::Duration::days(1 + (i / 3) as i64);
        if date.weekday().num_days_from_monday() >= 6 {
            continue;
        }
        let hour = 9 + (i % 3) * 3;
        appointments::save(
            conn,
            &appointments::AppointmentInput {
                id: None,
                client_id: Some(client_ids[(i as usize * 7) % client_ids.len()]),
                staff_id: staff[(i % 2) as usize],
                location_id: Some(loc),
                starts_at: format!("{}T{hour:02}:00", ds(date)),
                ends_at: None,
                notes: String::new(),
                lines: vec![appointments::AppointmentLine { service_id: menu[(i % 5) as usize].0, variant_ids: vec![], qty: 1 }],
                estimate_id: None,
                allow_overlap: true,
            },
        )?;
    }

    // Fictional competitors and observed prices
    for (i, (name, base)) in [("Example Hair Co.", 88), ("Sample Studio", 79), ("Demo Cuts", 72), ("Placeholder Salon", 99), ("Testwell Beauty", 84)].iter().enumerate() {
        let cid = market::save_competitor(
            conn,
            &market::CompetitorInput { id: None, name: name.to_string(), address: format!("{} Example Ave", 10 + i), city: "Springfield".into(), state: "WA".into(), postal_code: "98000".into(), latitude: Some(format!("{:.4}", 47.0 + i as f64 * 0.004)), longitude: Some("-122.9050".into()), website: String::new(), phone: String::new(), notes: "Fictional demo business".into() },
            "manual",
            None,
            "",
        )?;
        for (sid, label, mult) in [(root, "Root color", 100), (cut, "Women's cut", 70), (hl, "Partial foils", 175)] {
            market::save_observation(
                conn,
                &market::ObservationInput {
                    id: None,
                    competitor_id: cid,
                    service_id: Some(sid),
                    service_label: label.into(),
                    price: Decimal::from(*base * mult / 100),
                    price_type: if i == 2 && sid == hl { "starting_at".into() } else { "exact".into() },
                    price_max: None,
                    duration_min: None,
                    hair_length: String::new(),
                    stylist_level: String::new(),
                    inclusions: String::new(),
                    source_url: String::new(),
                    observed_on: ds(today - chrono::Duration::days(20 + i as i64 * 9)),
                    notes: "Fictional demo observation".into(),
                },
                "user_observed",
            )?;
        }
    }
    crate::db::audit(conn, "workspace", None, "demo_seed", "Demo data created", None)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::reports::{report, ReportFilter};

    #[test]
    fn demo_seed_is_consistent() {
        let conn = crate::db::open_memory();
        let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
        seed(&conn, today, 2, 3).unwrap();
        let r = report(&conn, &ReportFilter { from: "2026-08-01".into(), to: "2026-10-04".into(), ..Default::default() }, &["service".into()], Some("month"), false).unwrap();
        assert!(r.summary.sales_count > 100);
        let by: Decimal = r.groups["service"].iter().map(|g| g.revenue - g.refunds).sum();
        assert_eq!(by, r.summary.net_revenue);
        // every product's ledger reconciles with its on-hand figures
        for p in inventory::products(&conn, true).unwrap() {
            let rows = inventory::ledger(&conn, &inventory::LedgerFilter { product_id: Some(p.id), limit: Some(20000), ..Default::default() }).unwrap();
            assert_eq!(rows.iter().map(|r| r.qty_base).sum::<Decimal>(), p.on_hand_base, "{}", p.name);
            assert_eq!(rows.iter().map(|r| r.value).sum::<Decimal>(), p.value, "{}", p.name);
        }
    }

    /// Larger dataset for performance checks: `cargo test --release large_dataset -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn large_dataset() {
        let conn = crate::db::open_memory();
        let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
        let t = std::time::Instant::now();
        seed(&conn, today, 24, 16).unwrap();
        let lines: i64 = conn.query_row("SELECT COUNT(*) FROM sale_lines", [], |r| r.get(0)).unwrap();
        println!("seeded {lines} sale lines in {:?}", t.elapsed());
        let t = std::time::Instant::now();
        let r = report(&conn, &ReportFilter { from: "2024-10-01".into(), to: "2026-10-04".into(), ..Default::default() }, &["service".into(), "staff".into(), "profile".into()], Some("week"), true).unwrap();
        println!("two-year report ({} sales) in {:?}", r.summary.sales_count, t.elapsed());
        let t = std::time::Instant::now();
        let p = inventory::products(&conn, false).unwrap();
        let s = crate::db::sales::list(&conn, &crate::db::sales::SaleFilter::default()).unwrap();
        println!("products ({}) + sales list ({}) in {:?}", p.len(), s.len(), t.elapsed());
        assert!(lines > 15000);
    }
}
