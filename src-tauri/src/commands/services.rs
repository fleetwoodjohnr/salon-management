//! Services, recipes, estimates and bundles.

use crate::db::core::set_archived;
use crate::db::services::{self as q, BundleInput, BundleView, Estimate, ServiceInput, ServiceSummary, ServiceView, TaxContext};
use crate::error::AppResult;
use crate::workspace::AppState;
use rusqlite::Connection;
use tauri::State;

/// Sales-tax context for a service line at the business's primary location.
pub fn tax_for(conn: &Connection, s: &ServiceInput) -> AppResult<TaxContext> {
    crate::db::tax::context_for(conn, None, &s.tax_category)
}

#[tauri::command]
pub async fn services_list(state: State<'_, AppState>, include_archived: bool) -> AppResult<Vec<ServiceSummary>> {
    state.read(|c| q::list(c, include_archived, tax_for))
}

#[tauri::command]
pub async fn service_get(state: State<'_, AppState>, id: i64) -> AppResult<ServiceView> {
    state.read(|c| q::get(c, id))
}

#[tauri::command]
pub async fn service_save(state: State<'_, AppState>, service: ServiceInput) -> AppResult<ServiceView> {
    state.tx(|c| {
        let id = q::save(c, &service)?;
        q::get(c, id)
    })
}

#[tauri::command]
pub async fn service_archive(state: State<'_, AppState>, id: i64, archived: bool) -> AppResult<()> {
    state.tx(|c| set_archived(c, "services", "service", id, archived))
}

/// Live estimate for a (possibly unsaved) service and chosen variants.
#[tauri::command]
pub async fn service_estimate(state: State<'_, AppState>, service: ServiceInput, variant_ids: Vec<i64>) -> AppResult<Estimate> {
    state.read(|c| {
        let tax = tax_for(c, &service)?;
        q::estimate(c, &service, &variant_ids, tax)
    })
}

#[tauri::command]
pub async fn bundles_list(state: State<'_, AppState>, include_archived: bool) -> AppResult<Vec<BundleView>> {
    state.read(|c| q::bundles(c, include_archived, tax_for))
}

#[tauri::command]
pub async fn bundle_save(state: State<'_, AppState>, bundle: BundleInput) -> AppResult<i64> {
    state.tx(|c| q::save_bundle(c, &bundle))
}

#[tauri::command]
pub async fn bundle_archive(state: State<'_, AppState>, id: i64, archived: bool) -> AppResult<()> {
    state.tx(|c| set_archived(c, "bundles", "bundle", id, archived))
}

#[tauri::command]
pub async fn pricing_what_if(state: State<'_, AppState>, what_if: crate::db::pricing::WhatIf) -> AppResult<crate::db::pricing::WhatIfResult> {
    state.read(|c| crate::db::pricing::what_if(c, &what_if, tax_for))
}
