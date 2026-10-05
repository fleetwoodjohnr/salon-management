//! Services, variants, versioned recipes and live estimates (cost breakdown + pricing).

use super::{audit, dec, now_utc, opt_dec};
use crate::db::inventory::product;
use crate::db::profiles;
use crate::domain::costing::{service_cost, CostBreakdown, MaterialCost, TimeSpec};
use crate::domain::inventory::Stock;
use crate::domain::pricing::{PriceAnalysis, PriceParams, Targets};
use crate::domain::profile::{Position, ProfileData, RoundMode, TargetKind};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PricingOverrides {
    pub target_kind: Option<TargetKind>,
    pub target_pct: Option<Decimal>,
    pub position: Option<Position>,
    pub rounding_increment: Option<Decimal>,
    pub rounding_mode: Option<RoundMode>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VariantInput {
    pub id: Option<i64>,
    pub group_name: String,
    pub name: String,
    pub hands_on_delta: i64,
    pub processing_delta: i64,
    pub material_factor: Decimal,
    pub price_delta: Decimal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecipeLineInput {
    pub product_id: i64,
    pub qty: Decimal,
    pub unit: String,
    pub note: String,
}

/// Everything about a service that affects its estimate. Used both for saving and for live
/// previews of unsaved edits.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ServiceInput {
    pub id: Option<i64>,
    pub name: String,
    pub category: String,
    pub description: String,
    pub profile_id: Option<i64>,
    pub time: TimeSpec,
    pub is_addon: bool,
    pub waste_pct: Decimal,
    pub other_direct_cost: Decimal,
    pub other_direct_note: String,
    pub price: Option<Decimal>,
    pub price_note: String,
    pub overrides: PricingOverrides,
    pub tax_category: String,
    pub variants: Vec<VariantInput>,
    pub recipe: Vec<RecipeLineInput>,
    pub recipe_note: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct RecipeVersionInfo {
    pub id: i64,
    pub version: i64,
    pub note: String,
    pub created_at: String,
    pub lines: Vec<RecipeLineView>,
}

#[derive(Serialize, Clone, Debug)]
pub struct RecipeLineView {
    pub product_id: i64,
    pub product_name: String,
    pub qty: Decimal,
    pub unit: String,
    pub note: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct ServiceView {
    pub input: ServiceInput,
    pub archived: bool,
    pub price_set_at: Option<String>,
    pub recipe_version_id: Option<i64>,
    pub recipe_version: Option<i64>,
    pub recipe_versions: Vec<RecipeVersionInfo>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ServiceSummary {
    pub id: i64,
    pub name: String,
    pub category: String,
    pub is_addon: bool,
    pub profile_name: Option<String>,
    pub duration_min: i64,
    pub price: Option<Decimal>,
    pub estimated_cost: Option<Decimal>,
    pub margin_pct: Option<Decimal>,
    pub target_price: Option<Decimal>,
    /// "ok" | "below_target" | "below_cost" | "no_price" | "error"
    pub status: String,
    pub status_detail: Option<String>,
    pub archived: bool,
}

// ---------------------------------------------------------------- tax context

/// What pricing needs to know about sales tax for a line. `rate_pct` is None when the rate or the
/// taxability is not yet resolved — estimates then omit processing on tax and say so.
#[derive(Serialize, Clone, Debug)]
pub struct TaxContext {
    pub taxable: Option<bool>,
    pub rate_pct: Option<Decimal>,
    pub prices_include_tax: bool,
    pub label: String,
    pub resolved: bool,
}

// ---------------------------------------------------------------- read

fn load_variants(conn: &Connection, service_id: i64) -> AppResult<Vec<VariantInput>> {
    let mut st = conn.prepare(
        "SELECT id, group_name, name, hands_on_delta, processing_delta, material_factor, price_delta FROM service_variants
         WHERE service_id = ?1 AND archived_at IS NULL ORDER BY sort, id",
    )?;
    let rows = st.query_map([service_id], |r| {
        Ok(VariantInput {
            id: r.get(0)?,
            group_name: r.get(1)?,
            name: r.get(2)?,
            hands_on_delta: r.get(3)?,
            processing_delta: r.get(4)?,
            material_factor: dec(r, "material_factor")?,
            price_delta: dec(r, "price_delta")?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn recipe_lines(conn: &Connection, version_id: i64) -> AppResult<Vec<RecipeLineView>> {
    let mut st = conn.prepare(
        "SELECT rl.product_id, p.name, rl.qty, rl.unit, rl.note FROM recipe_lines rl JOIN products p ON p.id = rl.product_id
         WHERE rl.recipe_version_id = ?1 ORDER BY rl.sort, rl.id",
    )?;
    let rows = st.query_map([version_id], |r| {
        Ok(RecipeLineView { product_id: r.get(0)?, product_name: r.get(1)?, qty: dec(r, "qty")?, unit: r.get(3)?, note: r.get(4)? })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn recipe_versions(conn: &Connection, service_id: i64) -> AppResult<Vec<RecipeVersionInfo>> {
    let mut st = conn.prepare("SELECT id, version, note, created_at FROM recipe_versions WHERE service_id = ?1 ORDER BY version DESC")?;
    let heads: Vec<(i64, i64, String, String)> = st.query_map([service_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<Result<_, _>>()?;
    heads
        .into_iter()
        .map(|(id, version, note, created_at)| Ok(RecipeVersionInfo { id, version, note, created_at, lines: recipe_lines(conn, id)? }))
        .collect()
}

pub fn get(conn: &Connection, id: i64) -> AppResult<ServiceView> {
    let row = conn
        .query_row(
            "SELECT name, category, description, profile_id, hands_on_min, processing_min, setup_min, cleanup_min, is_addon, waste_pct,
                    other_direct_cost, other_direct_note, price, price_note, price_set_at, target_kind, target_pct, position,
                    rounding_increment, rounding_mode, tax_category, archived_at
             FROM services WHERE id = ?1",
            [id],
            |r| {
                let parse_enum = |s: Option<String>| s.map(|s| serde_json::Value::String(s));
                Ok((
                    ServiceInput {
                        id: Some(id),
                        name: r.get(0)?,
                        category: r.get(1)?,
                        description: r.get(2)?,
                        profile_id: r.get(3)?,
                        time: TimeSpec { hands_on_min: r.get(4)?, processing_min: r.get(5)?, setup_min: r.get(6)?, cleanup_min: r.get(7)? },
                        is_addon: r.get::<_, i64>(8)? != 0,
                        waste_pct: dec(r, "waste_pct")?,
                        other_direct_cost: dec(r, "other_direct_cost")?,
                        other_direct_note: r.get(11)?,
                        price: opt_dec(r, "price")?,
                        price_note: r.get(13)?,
                        overrides: PricingOverrides {
                            target_kind: parse_enum(r.get(15)?).and_then(|v| serde_json::from_value(v).ok()),
                            target_pct: opt_dec(r, "target_pct")?,
                            position: parse_enum(r.get(17)?).and_then(|v| serde_json::from_value(v).ok()),
                            rounding_increment: opt_dec(r, "rounding_increment")?,
                            rounding_mode: parse_enum(r.get(19)?).and_then(|v| serde_json::from_value(v).ok()),
                        },
                        tax_category: r.get(20)?,
                        variants: vec![],
                        recipe: vec![],
                        recipe_note: String::new(),
                    },
                    r.get::<_, Option<String>>(14)?,
                    r.get::<_, Option<String>>(21)?.is_some(),
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("Service {id} not found.")))?;
    let (mut input, price_set_at, archived) = row;
    input.variants = load_variants(conn, id)?;
    let versions = recipe_versions(conn, id)?;
    if let Some(v) = versions.first() {
        input.recipe = v.lines.iter().map(|l| RecipeLineInput { product_id: l.product_id, qty: l.qty, unit: l.unit.clone(), note: l.note.clone() }).collect();
    }
    Ok(ServiceView {
        archived,
        price_set_at,
        recipe_version_id: versions.first().map(|v| v.id),
        recipe_version: versions.first().map(|v| v.version),
        recipe_versions: versions,
        input,
    })
}

// ---------------------------------------------------------------- write

fn enum_str<T: Serialize>(v: &Option<T>) -> AppResult<Option<String>> {
    Ok(match v {
        Some(x) => serde_json::to_value(x)?.as_str().map(String::from),
        None => None,
    })
}

fn validate(conn: &Connection, s: &ServiceInput) -> AppResult<()> {
    if s.name.trim().is_empty() {
        return Err(AppError::invalid("name", "Name the service."));
    }
    s.time.validate()?;
    crate::domain::money::pct_0_100("waste_pct", "Waste allowance", s.waste_pct)?;
    crate::domain::money::non_negative("other_direct_cost", "Other direct cost", s.other_direct_cost)?;
    if let Some(p) = s.price {
        crate::domain::money::non_negative("price", "Price", p)?;
        if p != p.round_dp(2) {
            return Err(AppError::invalid("price", "Price must be in whole cents."));
        }
    }
    if let Some(pid) = s.profile_id {
        profiles::get(conn, pid)?;
    }
    if let Some(t) = s.overrides.target_pct {
        match s.overrides.target_kind.unwrap_or(TargetKind::Margin) {
            TargetKind::Margin => crate::domain::money::pct_below_100("overrides.target_pct", "Target margin", t)?,
            TargetKind::Markup => crate::domain::money::non_negative("overrides.target_pct", "Target markup", t)?,
        };
    }
    for (i, v) in s.variants.iter().enumerate() {
        if v.group_name.trim().is_empty() || v.name.trim().is_empty() {
            return Err(AppError::invalid(&format!("variants.{i}.name"), "Each variant needs a group (e.g. Hair length) and a name (e.g. Long)."));
        }
        if v.material_factor < Decimal::ZERO {
            return Err(AppError::invalid(&format!("variants.{i}.material_factor"), "Material factor can't be negative."));
        }
    }
    for (i, l) in s.recipe.iter().enumerate() {
        if l.qty <= Decimal::ZERO {
            return Err(AppError::invalid(&format!("recipe.{i}.qty"), "Recipe quantities must be more than zero."));
        }
        let p = product(conn, l.product_id)?;
        p.units().to_base(l.qty, &l.unit).map_err(|e| AppError::invalid(&format!("recipe.{i}.unit"), e.to_string()))?;
    }
    Ok(())
}

pub fn save(conn: &Connection, s: &ServiceInput) -> AppResult<i64> {
    validate(conn, s)?;
    let prev = s.id.map(|id| get(conn, id)).transpose()?;
    let price_changed = prev.as_ref().map(|p| p.input.price != s.price).unwrap_or(s.price.is_some());
    let p = params![
        s.name.trim(),
        s.category.trim(),
        s.description,
        s.profile_id,
        s.time.hands_on_min,
        s.time.processing_min,
        s.time.setup_min,
        s.time.cleanup_min,
        s.is_addon as i64,
        s.waste_pct.to_string(),
        s.other_direct_cost.to_string(),
        s.other_direct_note,
        s.price.map(|d| d.to_string()),
        s.price_note,
        enum_str(&s.overrides.target_kind)?,
        s.overrides.target_pct.map(|d| d.to_string()),
        enum_str(&s.overrides.position)?,
        s.overrides.rounding_increment.map(|d| d.to_string()),
        enum_str(&s.overrides.rounding_mode)?,
        s.tax_category,
        now_utc(),
    ];
    let id = match s.id {
        Some(id) => {
            conn.execute(
                "UPDATE services SET name=?1, category=?2, description=?3, profile_id=?4, hands_on_min=?5, processing_min=?6, setup_min=?7,
                 cleanup_min=?8, is_addon=?9, waste_pct=?10, other_direct_cost=?11, other_direct_note=?12, price=?13, price_note=?14,
                 target_kind=?15, target_pct=?16, position=?17, rounding_increment=?18, rounding_mode=?19, tax_category=?20, updated_at=?21
                 WHERE id = ?22",
                rusqlite::params_from_iter(p.iter().copied().chain(std::iter::once(&id as &dyn rusqlite::ToSql))),
            )?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO services (name, category, description, profile_id, hands_on_min, processing_min, setup_min, cleanup_min, is_addon,
                 waste_pct, other_direct_cost, other_direct_note, price, price_note, target_kind, target_pct, position, rounding_increment,
                 rounding_mode, tax_category, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21)",
                p,
            )?;
            conn.last_insert_rowid()
        }
    };
    if price_changed {
        conn.execute("UPDATE services SET price_set_at = ?1 WHERE id = ?2", params![now_utc(), id])?;
    }
    // Variants: update in place, add new, archive removed (past sales keep their own snapshot).
    let keep: Vec<i64> = s.variants.iter().filter_map(|v| v.id).collect();
    for old in load_variants(conn, id)? {
        if !keep.contains(&old.id.unwrap()) {
            conn.execute("UPDATE service_variants SET archived_at = ?1 WHERE id = ?2", params![now_utc(), old.id])?;
        }
    }
    for (i, v) in s.variants.iter().enumerate() {
        let vp = params![v.group_name.trim(), v.name.trim(), v.hands_on_delta, v.processing_delta, v.material_factor.to_string(), v.price_delta.to_string(), i as i64];
        match v.id {
            Some(vid) => {
                conn.execute(
                    "UPDATE service_variants SET group_name=?1, name=?2, hands_on_delta=?3, processing_delta=?4, material_factor=?5, price_delta=?6, sort=?7
                     WHERE id=?8 AND service_id=?9",
                    rusqlite::params_from_iter(vp.iter().copied().chain([&vid as &dyn rusqlite::ToSql, &id])),
                )?;
            }
            None => {
                conn.execute(
                    "INSERT INTO service_variants (group_name, name, hands_on_delta, processing_delta, material_factor, price_delta, sort, service_id)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                    rusqlite::params_from_iter(vp.iter().copied().chain(std::iter::once(&id as &dyn rusqlite::ToSql))),
                )?;
            }
        }
    }
    // Recipe: a new version only when the lines actually change.
    let current: Vec<RecipeLineInput> = prev.as_ref().map(|p| p.input.recipe.clone()).unwrap_or_default();
    let has_version = prev.as_ref().is_some_and(|p| p.recipe_version.is_some());
    if current != s.recipe || (!has_version && !s.recipe.is_empty()) {
        let next: i64 = conn.query_row("SELECT COALESCE(MAX(version), 0) + 1 FROM recipe_versions WHERE service_id = ?1", [id], |r| r.get(0))?;
        conn.execute("INSERT INTO recipe_versions (service_id, version, note) VALUES (?1, ?2, ?3)", params![id, next, s.recipe_note.trim()])?;
        let vid = conn.last_insert_rowid();
        for (i, l) in s.recipe.iter().enumerate() {
            conn.execute(
                "INSERT INTO recipe_lines (recipe_version_id, product_id, qty, unit, note, sort) VALUES (?1,?2,?3,?4,?5,?6)",
                params![vid, l.product_id, l.qty.to_string(), l.unit, l.note, i as i64],
            )?;
        }
    }
    audit(
        conn,
        "service",
        Some(id),
        if s.id.is_some() { "update" } else { "create" },
        &format!("Service \"{}\" saved{}", s.name.trim(), if price_changed { format!(" (price {})", s.price.map(|p| format!("${p}")).unwrap_or("cleared".into())) } else { String::new() }),
        Some(serde_json::to_value(s)?),
    )?;
    Ok(id)
}

// ---------------------------------------------------------------- estimates

#[derive(Serialize, Clone, Debug)]
pub struct PricingView {
    pub params: PriceParams,
    pub targets: Option<Targets>,
    pub targets_error: Option<String>,
    /// At the chosen price (base price + variant price adjustments), if any
    pub at_price: Option<PriceAnalysis>,
    pub chosen_price: Option<Decimal>,
    pub earnings_per_working_hour: Option<Decimal>,
    pub earnings_per_occupied_hour: Option<Decimal>,
    pub tax: TaxContext,
    pub position: Position,
    pub warnings: Vec<String>,
    /// "ok" | "below_target" | "below_cost" | "no_price"
    pub status: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Estimate {
    pub profile_id: Option<i64>,
    pub profile_name: Option<String>,
    pub profile_version_id: Option<i64>,
    pub cost: Option<CostBreakdown>,
    pub cost_error: Option<String>,
    pub pricing: Option<PricingView>,
}

/// Apply selected variants (one per group) to time, material factor and price.
pub fn apply_variants(s: &ServiceInput, variant_ids: &[i64]) -> AppResult<(TimeSpec, Decimal, Decimal, Vec<String>)> {
    let mut time = s.time;
    let mut factor = Decimal::ONE;
    let mut price_delta = Decimal::ZERO;
    let mut names = Vec::new();
    let mut groups = std::collections::HashSet::new();
    for vid in variant_ids {
        let v = s.variants.iter().find(|v| v.id == Some(*vid)).ok_or_else(|| AppError::msg("Unknown variant for this service."))?;
        if !groups.insert(v.group_name.clone()) {
            return Err(AppError::msg(format!("Choose only one option for {}.", v.group_name)));
        }
        time.hands_on_min += v.hands_on_delta;
        time.processing_min += v.processing_delta;
        factor *= v.material_factor;
        price_delta += v.price_delta;
        names.push(format!("{}: {}", v.group_name, v.name));
    }
    if time.hands_on_min < 0 || time.processing_min < 0 {
        return Err(AppError::msg("Variant time adjustments make the service time negative."));
    }
    Ok((time, factor, price_delta, names))
}

/// Material cost lines at current average cost.
pub fn material_lines(conn: &Connection, recipe: &[RecipeLineInput], factor: Decimal) -> AppResult<Vec<MaterialCost>> {
    recipe
        .iter()
        .map(|l| {
            let p = product(conn, l.product_id)?;
            let pu = p.units();
            let qty = l.qty * factor;
            let qb = pu.to_base(qty, &l.unit)?;
            let stock = Stock { qty: p.on_hand_base, value: p.value, last_unit_cost: last_unit_cost(conn, p.id)? };
            let cost = stock.cost_of(qb);
            Ok(MaterialCost {
                product_id: p.id,
                name: if p.brand.is_empty() { p.name.clone() } else { format!("{} {}", p.brand, p.name) },
                qty,
                unit: l.unit.clone(),
                qty_base: qb,
                base_unit: p.base_unit.clone(),
                unit_cost: stock.avg_unit_cost(),
                cost: cost.unwrap_or_default(),
                warning: cost.is_none().then(|| format!("{} has no purchase cost yet, so it's counted as $0. Receive it in Inventory to cost it.", p.name)),
            })
        })
        .collect()
}

fn last_unit_cost(conn: &Connection, product_id: i64) -> AppResult<Option<Decimal>> {
    Ok(conn
        .query_row("SELECT last_unit_cost FROM products WHERE id = ?1", [product_id], |r| r.get::<_, Option<String>>(0))?
        .and_then(|s| s.parse().ok()))
}

pub fn pricing_params(profile: &ProfileData, commission_pct: Decimal, ov: &PricingOverrides, fixed_cost: Decimal, tax: &TaxContext) -> PriceParams {
    PriceParams {
        fixed_cost,
        commission_pct,
        processing_pct: profile.processing_pct,
        processing_fixed: profile.processing_fixed,
        card_share_pct: profile.card_share_pct,
        tax_rate_pct: if tax.taxable == Some(false) { Some(Decimal::ZERO) } else { tax.rate_pct },
        prices_include_tax: tax.prices_include_tax,
        target_kind: ov.target_kind.unwrap_or(profile.target_kind),
        target_pct: ov.target_pct.unwrap_or(profile.target_pct),
        rounding_increment: ov.rounding_increment.unwrap_or(profile.rounding_increment),
        rounding_mode: ov.rounding_mode.unwrap_or(profile.rounding_mode),
    }
}

/// Estimate a (possibly unsaved) service with the chosen variants.
pub fn estimate(conn: &Connection, s: &ServiceInput, variant_ids: &[i64], tax: TaxContext) -> AppResult<Estimate> {
    match s.profile_id {
        Some(pid) => estimate_with_profile(conn, s, variant_ids, tax, &profiles::get(conn, pid)?),
        None => Ok(Estimate {
            profile_id: None,
            profile_name: None,
            profile_version_id: None,
            cost: None,
            cost_error: Some("Choose a work profile to cost labor and overhead.".into()),
            pricing: None,
        }),
    }
}

/// Estimate with an explicit (possibly what-if) work profile.
pub fn estimate_with_profile(conn: &Connection, s: &ServiceInput, variant_ids: &[i64], tax: TaxContext, prof: &profiles::ProfileView) -> AppResult<Estimate> {
    let pid = prof.id;
    let prof = prof.clone();
    let rates = match prof.data.rates() {
        Ok(r) => r,
        Err(e) => {
            return Ok(Estimate {
                profile_id: Some(pid),
                profile_name: Some(prof.name),
                profile_version_id: Some(prof.version_id),
                cost: None,
                cost_error: Some(format!("The work profile needs fixing first: {e}")),
                pricing: None,
            })
        }
    };
    let (time, factor, price_delta, _) = apply_variants(s, variant_ids)?;
    let lines = material_lines(conn, &s.recipe, factor)?;
    let cost = match service_cost(&rates, prof.data.overhead_basis, time, lines, s.waste_pct, s.other_direct_cost) {
        Ok(c) => c,
        Err(e) => {
            return Ok(Estimate { profile_id: Some(pid), profile_name: Some(prof.name), profile_version_id: Some(prof.version_id), cost: None, cost_error: Some(e.to_string()), pricing: None })
        }
    };
    let params = pricing_params(&prof.data, rates.commission_pct, &s.overrides, cost.fixed_total, &tax);
    let mut warnings = Vec::new();
    if !tax.resolved {
        warnings.push(format!("Sales tax for this service isn't resolved ({}). Card fees on tax are left out of these figures until it is.", tax.label));
    }
    let (targets, targets_error) = match params.targets() {
        Ok(t) => (Some(t), None),
        Err(e) => (None, Some(e.to_string())),
    };
    let chosen = s.price.map(|p| p + price_delta);
    let at_price = chosen.map(|p| params.analyze(p));
    let mut status = "no_price".to_string();
    if let (Some(a), Some(t)) = (&at_price, &targets) {
        status = if a.list_price < t.break_even {
            warnings.push(format!("The price is below cost: each one loses ${}.", (-a.profit).round_dp(2)));
            "below_cost".into()
        } else if !params.meets_target(a.list_price) {
            warnings.push(format!(
                "The price misses the {} {}% target (needs ${}).",
                if params.target_kind == TargetKind::Margin { "margin" } else { "markup" },
                params.target_pct.normalize(),
                t.target_price
            ));
            "below_target".into()
        } else {
            "ok".into()
        };
        if status != "ok" && s.price_note.trim().is_empty() {
            warnings.push("Add a note explaining why this price is below target (an explicit override).".into());
        }
    }
    let earnings = at_price.as_ref().map(|a| a.profit + cost.labor);
    let per = |h: Decimal| earnings.filter(|_| h > Decimal::ZERO).map(|e| e / h);
    let pricing = PricingView {
        earnings_per_working_hour: per(cost.working_hours),
        earnings_per_occupied_hour: per(cost.occupied_hours),
        position: s.overrides.position.unwrap_or(prof.data.position),
        params,
        targets,
        targets_error,
        at_price,
        chosen_price: chosen,
        tax,
        warnings,
        status,
    };
    Ok(Estimate { profile_id: Some(pid), profile_name: Some(prof.name), profile_version_id: Some(prof.version_id), cost: Some(cost), cost_error: None, pricing: Some(pricing) })
}

pub fn list(conn: &Connection, include_archived: bool, tax_for: impl Fn(&Connection, &ServiceInput) -> AppResult<TaxContext>) -> AppResult<Vec<ServiceSummary>> {
    let ids: Vec<(i64, bool)> = {
        let mut st = conn.prepare("SELECT id, archived_at IS NOT NULL FROM services WHERE (?1 OR archived_at IS NULL) ORDER BY category, name COLLATE NOCASE")?;
        let rows = st.query_map([include_archived], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut out = Vec::new();
    for (id, archived) in ids {
        let v = get(conn, id)?;
        let e = estimate(conn, &v.input, &[], tax_for(conn, &v.input)?)?;
        let pr = e.pricing.as_ref();
        out.push(ServiceSummary {
            id,
            name: v.input.name.clone(),
            category: v.input.category.clone(),
            is_addon: v.input.is_addon,
            profile_name: e.profile_name.clone(),
            duration_min: v.input.time.occupied_min(),
            price: v.input.price,
            estimated_cost: pr.and_then(|p| p.at_price.as_ref().map(|a| a.total_cost)).or(e.cost.as_ref().map(|c| c.fixed_total)),
            margin_pct: pr.and_then(|p| p.at_price.as_ref().and_then(|a| a.margin_pct)),
            target_price: pr.and_then(|p| p.targets.as_ref().map(|t| t.target_price)),
            status: if e.cost_error.is_some() { "error".into() } else { pr.map(|p| p.status.clone()).unwrap_or("no_price".into()) },
            status_detail: e.cost_error.clone().or_else(|| pr.and_then(|p| p.targets_error.clone())),
            archived,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------- bundles

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BundleItem {
    pub service_id: i64,
    pub qty: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BundleInput {
    pub id: Option<i64>,
    pub name: String,
    pub description: String,
    pub price: Option<Decimal>,
    pub items: Vec<BundleItem>,
}

#[derive(Serialize, Clone, Debug)]
pub struct BundleLine {
    pub service_id: i64,
    pub name: String,
    pub qty: i64,
    pub list_price: Option<Decimal>,
    /// Share of the bundle price allocated to this line (by list price)
    pub allocated_price: Option<Decimal>,
    pub total_cost: Option<Decimal>,
}

#[derive(Serialize, Clone, Debug)]
pub struct BundleView {
    pub input: BundleInput,
    pub archived: bool,
    pub lines: Vec<BundleLine>,
    /// Sum of the services' own prices × quantity
    pub separate_total: Option<Decimal>,
    pub total_cost: Option<Decimal>,
    pub profit: Option<Decimal>,
    pub margin_pct: Option<Decimal>,
    pub warnings: Vec<String>,
}

pub fn save_bundle(conn: &Connection, b: &BundleInput) -> AppResult<i64> {
    if b.name.trim().is_empty() {
        return Err(AppError::invalid("name", "Name the bundle."));
    }
    if b.items.is_empty() {
        return Err(AppError::invalid("items", "Add at least one service."));
    }
    if let Some(p) = b.price {
        crate::domain::money::non_negative("price", "Price", p)?;
        if p != p.round_dp(2) {
            return Err(AppError::invalid("price", "Price must be in whole cents."));
        }
    }
    for it in &b.items {
        if it.qty < 1 {
            return Err(AppError::invalid("items", "Quantities must be at least 1."));
        }
        get(conn, it.service_id)?;
    }
    let id = match b.id {
        Some(id) => {
            conn.execute("UPDATE bundles SET name=?1, description=?2, price=?3 WHERE id=?4", params![b.name.trim(), b.description, b.price.map(|d| d.to_string()), id])?;
            conn.execute("DELETE FROM bundle_items WHERE bundle_id = ?1", [id])?;
            id
        }
        None => {
            conn.execute("INSERT INTO bundles (name, description, price) VALUES (?1, ?2, ?3)", params![b.name.trim(), b.description, b.price.map(|d| d.to_string())])?;
            conn.last_insert_rowid()
        }
    };
    for it in &b.items {
        conn.execute("INSERT INTO bundle_items (bundle_id, service_id, qty) VALUES (?1, ?2, ?3)", params![id, it.service_id, it.qty])?;
    }
    audit(conn, "bundle", Some(id), if b.id.is_some() { "update" } else { "create" }, &format!("Bundle \"{}\" saved", b.name.trim()), Some(serde_json::to_value(b)?))?;
    Ok(id)
}

/// Bundles with their economics: the bundle price is allocated to each service in proportion to
/// its own list price, and each share is analyzed with that service's own costs and fees.
pub fn bundles(conn: &Connection, include_archived: bool, tax_for: impl Fn(&Connection, &ServiceInput) -> AppResult<TaxContext>) -> AppResult<Vec<BundleView>> {
    let heads: Vec<(i64, String, String, Option<String>, bool)> = {
        let mut st = conn.prepare("SELECT id, name, description, price, archived_at IS NOT NULL FROM bundles WHERE (?1 OR archived_at IS NULL) ORDER BY name")?;
        let rows = st.query_map([include_archived], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut out = Vec::new();
    for (id, name, description, price, archived) in heads {
        let price: Option<Decimal> = price.and_then(|p| p.parse().ok());
        let items: Vec<BundleItem> = {
            let mut st = conn.prepare("SELECT service_id, qty FROM bundle_items WHERE bundle_id = ?1 ORDER BY id")?;
            let rows = st.query_map([id], |r| Ok(BundleItem { service_id: r.get(0)?, qty: r.get(1)? }))?;
            rows.collect::<Result<_, _>>()?
        };
        let mut warnings = Vec::new();
        let services: Vec<ServiceView> = items.iter().map(|it| get(conn, it.service_id)).collect::<AppResult<_>>()?;
        let separate: Option<Decimal> = items.iter().zip(&services).map(|(it, s)| s.input.price.map(|p| p * Decimal::from(it.qty))).sum();
        if separate.is_none() {
            warnings.push("Some services have no price, so the bundle price can't be split between them.".into());
        }
        let mut lines = Vec::new();
        let mut total_cost = Some(Decimal::ZERO);
        for (it, s) in items.iter().zip(&services) {
            let share = match (price, separate, s.input.price) {
                (Some(bp), Some(sep), Some(sp)) if !sep.is_zero() => Some(bp * sp * Decimal::from(it.qty) / sep),
                _ => None,
            };
            let mut cost = None;
            if let Some(share) = share {
                let mut each = s.input.clone();
                each.price = Some(share / Decimal::from(it.qty));
                let e = estimate(conn, &each, &[], tax_for(conn, &each)?)?;
                cost = e.pricing.and_then(|p| p.at_price).map(|a| a.total_cost * Decimal::from(it.qty));
                if let Some(err) = e.cost_error {
                    warnings.push(format!("{}: {err}", s.input.name));
                }
            }
            total_cost = match (total_cost, cost) {
                (Some(t), Some(c)) => Some(t + c),
                _ => None,
            };
            lines.push(BundleLine { service_id: it.service_id, name: s.input.name.clone(), qty: it.qty, list_price: s.input.price, allocated_price: share, total_cost: cost });
        }
        let profit = match (price, total_cost) {
            (Some(p), Some(c)) => Some(p - c),
            _ => None,
        };
        let margin_pct = match (price, profit) {
            (Some(p), Some(pr)) if !p.is_zero() => Some(pr / p * Decimal::ONE_HUNDRED),
            _ => None,
        };
        if profit.is_some_and(|p| p < Decimal::ZERO) {
            warnings.push("This bundle loses money at its price.".into());
        }
        out.push(BundleView {
            input: BundleInput { id: Some(id), name, description, price, items },
            archived,
            lines,
            separate_total: separate,
            total_cost,
            profit,
            margin_pct,
            warnings,
        });
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::db::inventory::tests::{buy, product_input};
    use crate::db::inventory::save_product;
    use crate::db::open_memory;
    use crate::domain::profile::sample;
    use rust_decimal_macros::dec;

    fn unknown_tax() -> TaxContext {
        TaxContext { taxable: None, rate_pct: None, prices_include_tax: false, label: "not set up".into(), resolved: false }
    }

    pub(crate) fn service_input(profile_id: i64, product_id: i64) -> ServiceInput {
        ServiceInput {
            id: None,
            name: "Root touch-up".into(),
            category: "Color".into(),
            description: String::new(),
            profile_id: Some(profile_id),
            time: TimeSpec { hands_on_min: 45, processing_min: 35, setup_min: 5, cleanup_min: 10 },
            is_addon: false,
            waste_pct: dec!(10),
            other_direct_cost: dec!(1.25),
            other_direct_note: "gloves, foil".into(),
            price: Some(dec!(95)),
            price_note: String::new(),
            overrides: PricingOverrides::default(),
            tax_category: "service".into(),
            variants: vec![VariantInput { id: None, group_name: "Hair length".into(), name: "Long".into(), hands_on_delta: 15, processing_delta: 0, material_factor: dec!(1.5), price_delta: dec!(20) }],
            recipe: vec![RecipeLineInput { product_id, qty: dec!(2), unit: "fl_oz".into(), note: String::new() }],
            recipe_note: "initial".into(),
        }
    }

    #[test]
    fn save_estimate_and_recipe_versions() {
        let conn = open_memory();
        let prof = profiles::save(&conn, None, "Me", &sample()).unwrap();
        let dev = save_product(&conn, &product_input("Developer", "fl_oz")).unwrap();
        buy(&conn, dev, dec!(32), "fl_oz", dec!(24));
        let mut s = service_input(prof, dev);
        let id = save(&conn, &s).unwrap();
        let v = get(&conn, id).unwrap();
        assert_eq!(v.recipe_version, Some(1));
        assert_eq!(v.input.variants.len(), 1);

        let e = estimate(&conn, &v.input, &[], unknown_tax()).unwrap();
        let c = e.cost.unwrap();
        assert_eq!(c.materials, dec!(1.5));
        assert_eq!(c.waste, dec!(0.15));
        // labor: 60 working minutes at $40 → $40
        assert_eq!(c.labor, dec!(40));
        let p = e.pricing.unwrap();
        assert!(!p.tax.resolved);
        assert!(p.warnings.iter().any(|w| w.contains("isn't resolved")));

        // Long hair variant: 1.5× material, +15 min, +$20
        let long = v.input.variants[0].id.unwrap();
        let e2 = estimate(&conn, &v.input, &[long], unknown_tax()).unwrap();
        assert_eq!(e2.cost.unwrap().materials, dec!(2.25));
        assert_eq!(e2.pricing.unwrap().chosen_price, Some(dec!(115)));

        // Saving without recipe changes keeps the version; changing it adds one.
        s.id = Some(id);
        s.variants = v.input.variants.clone();
        save(&conn, &s).unwrap();
        assert_eq!(get(&conn, id).unwrap().recipe_version, Some(1));
        s.recipe[0].qty = dec!(3);
        save(&conn, &s).unwrap();
        let v3 = get(&conn, id).unwrap();
        assert_eq!(v3.recipe_version, Some(2));
        assert_eq!(v3.recipe_versions[1].lines[0].qty, dec!(2)); // old version intact
    }

    #[test]
    fn below_target_needs_note() {
        let conn = open_memory();
        let prof = profiles::save(&conn, None, "Me", &sample()).unwrap();
        let dev = save_product(&conn, &product_input("Developer", "fl_oz")).unwrap();
        buy(&conn, dev, dec!(32), "fl_oz", dec!(24));
        let mut s = service_input(prof, dev);
        s.price = Some(dec!(20));
        let e = estimate(&conn, &s, &[], unknown_tax()).unwrap();
        let p = e.pricing.unwrap();
        assert_eq!(p.status, "below_cost");
        assert!(p.warnings.iter().any(|w| w.contains("Add a note")));
    }

    #[test]
    fn bundle_price_is_split_by_list_price() {
        let conn = open_memory();
        let prof = profiles::save(&conn, None, "Me", &sample()).unwrap();
        let dev = save_product(&conn, &product_input("Developer", "fl_oz")).unwrap();
        buy(&conn, dev, dec!(32), "fl_oz", dec!(24));
        let mut a = service_input(prof, dev);
        a.price = Some(dec!(100));
        let a = save(&conn, &a).unwrap();
        let mut b = service_input(prof, dev);
        b.name = "Gloss".into();
        b.price = Some(dec!(50));
        let b = save(&conn, &b).unwrap();
        save_bundle(&conn, &BundleInput { id: None, name: "Color + gloss".into(), description: String::new(), price: Some(dec!(135)), items: vec![BundleItem { service_id: a, qty: 1 }, BundleItem { service_id: b, qty: 1 }] }).unwrap();
        let v = &bundles(&conn, false, |_, _| Ok(unknown_tax())).unwrap()[0];
        assert_eq!(v.separate_total, Some(dec!(150)));
        assert_eq!(v.lines[0].allocated_price, Some(dec!(90)));
        assert_eq!(v.lines[1].allocated_price, Some(dec!(45)));
        assert!(v.total_cost.is_some());
    }

    #[test]
    fn incompatible_recipe_unit_rejected() {
        let conn = open_memory();
        let prof = profiles::save(&conn, None, "Me", &sample()).unwrap();
        let dev = save_product(&conn, &product_input("Developer", "fl_oz")).unwrap();
        let mut s = service_input(prof, dev);
        s.recipe[0].unit = "oz_wt".into();
        let e = save(&conn, &s).unwrap_err().to_string();
        assert!(e.contains("density"), "{e}");
    }
}
