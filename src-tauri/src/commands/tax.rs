//! Sales-tax settings (rates, taxability) per business location.

use crate::db::core::{get_business, location, Location};
use crate::db::tax::{self as q, RateSet, RateSetInput, TaxCategory, TaxabilityRule};
use crate::error::{AppError, AppResult};
use crate::workspace::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct TaxOverview {
    pub location: Option<Location>,
    pub rate: Option<RateSet>,
    pub rules: Vec<TaxabilityRule>,
    pub prices_include_tax: bool,
    /// Plain-language list of what still blocks finalizing sales.
    pub unresolved: Vec<String>,
}

#[tauri::command]
pub async fn tax_overview(state: State<'_, AppState>, location_id: Option<i64>) -> AppResult<TaxOverview> {
    state.read(|c| {
        let biz = get_business(c)?;
        let Some(loc_id) = location_id.or(biz.primary_location_id) else {
            return Ok(TaxOverview {
                location: None,
                rate: None,
                rules: vec![],
                prices_include_tax: biz.prices_include_tax,
                unresolved: vec!["Add a business location in Settings first.".into()],
            });
        };
        let loc = location(c, loc_id)?;
        let rate = q::current_rate(c, loc_id)?;
        let rules = q::rules(c, loc_id)?;
        let mut unresolved = Vec::new();
        for r in &rules {
            if r.status == "unknown" {
                unresolved.push(format!("Decide whether {} are taxable.", r.category_label.to_lowercase()));
            }
        }
        if rate.is_none() && rules.iter().any(|r| r.status == "taxable") {
            unresolved.push("Set a sales tax rate for this location.".into());
        }
        Ok(TaxOverview { location: Some(loc), rate, rules, prices_include_tax: biz.prices_include_tax, unresolved })
    })
}

#[tauri::command]
pub async fn tax_categories(state: State<'_, AppState>) -> AppResult<Vec<TaxCategory>> {
    state.read(q::categories)
}

#[tauri::command]
pub async fn tax_category_add(state: State<'_, AppState>, label: String, applies_to: String) -> AppResult<String> {
    state.tx(|c| q::add_category(c, &label, &applies_to))
}

#[tauri::command]
pub async fn tax_rule_set(state: State<'_, AppState>, location_id: i64, category_code: String, status: String, basis: String) -> AppResult<()> {
    state.tx(|c| q::set_rule(c, location_id, &category_code, &status, &basis))
}

#[tauri::command]
pub async fn tax_rate_save(state: State<'_, AppState>, rate: RateSetInput) -> AppResult<RateSet> {
    if rate.source != "manual" {
        return Err(AppError::msg("Provider rates are saved through the lookup, not entered by hand."));
    }
    state.tx(|c| {
        let id = q::save_rate(c, &rate)?;
        q::rate_by_id(c, id)
    })
}

#[tauri::command]
pub async fn tax_rate_clear(state: State<'_, AppState>, location_id: i64, note: String) -> AppResult<()> {
    state.tx(|c| q::clear_rate(c, location_id, &note))
}

#[tauri::command]
pub async fn tax_rate_history(state: State<'_, AppState>, location_id: i64) -> AppResult<Vec<RateSet>> {
    state.read(|c| q::rate_history(c, location_id))
}
