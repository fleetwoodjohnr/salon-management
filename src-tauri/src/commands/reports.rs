//! Reports, dashboard views and widgets, market research, CSV export.

use crate::db::dashboards::{self, Dashboard};
use crate::db::inventory::{self, ReorderItem};
use crate::db::market::{self, Competitor, CompetitorInput, EvidenceQuery, MarketEvidence, ObsImportRow, Observation, ObservationInput, ObservationMapping};
use crate::db::reports::{self, LineFact, RefundFact, Report, ReportFilter};
use crate::db::appointments::{self, AppointmentView};
use crate::db::core::{get_business, location};
use crate::error::{AppError, AppResult};
use crate::providers::{bls, census, overpass};
use crate::workspace::AppState;
use rust_decimal::Decimal;
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub async fn report_run(state: State<'_, AppState>, filter: ReportFilter, groups: Vec<String>, trend_by: Option<String>, compare: bool) -> AppResult<Report> {
    state.read(|c| reports::report(c, &filter, &groups, trend_by.as_deref(), compare))
}

#[derive(Serialize)]
pub struct Drill {
    pub lines: Vec<LineFact>,
    pub refunds: Vec<RefundFact>,
}

#[tauri::command]
pub async fn report_drill(state: State<'_, AppState>, filter: ReportFilter) -> AppResult<Drill> {
    state.read(|c| Ok(Drill { lines: reports::line_facts(c, &filter)?, refunds: reports::refund_facts(c, &filter)? }))
}

#[tauri::command]
pub async fn dashboards_list(state: State<'_, AppState>) -> AppResult<Vec<Dashboard>> {
    state.tx(dashboards::list)
}
#[tauri::command]
pub async fn dashboard_save(state: State<'_, AppState>, dashboard: Dashboard) -> AppResult<i64> {
    state.tx(|c| dashboards::save(c, &dashboard))
}
#[tauri::command]
pub async fn dashboard_delete(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    state.tx(|c| dashboards::delete(c, id))
}

#[derive(Serialize)]
pub struct MarketComparison {
    pub service_id: i64,
    pub service_name: String,
    pub your_price: Option<Decimal>,
    pub median: Option<Decimal>,
    pub q1: Option<Decimal>,
    pub q3: Option<Decimal>,
    pub n: usize,
    pub quality: String,
}

#[derive(Serialize)]
pub struct Snapshot {
    pub inventory_value: Decimal,
    pub products: usize,
    pub low_stock: Vec<ReorderItem>,
    pub upcoming: Vec<AppointmentView>,
    pub pricing_alerts: Vec<crate::db::services::ServiceSummary>,
    pub market: Vec<MarketComparison>,
}

/// Point-in-time figures that don't depend on the date filter.
#[tauri::command]
pub async fn dashboard_snapshot(state: State<'_, AppState>, now: String, until: String) -> AppResult<Snapshot> {
    state.read(|c| {
        let products = inventory::products(c, false)?;
        let services = crate::db::services::list(c, false, crate::commands::services::tax_for)?;
        let today = &now[..10];
        let mut market_rows = Vec::new();
        for s in &services {
            let q = EvidenceQuery { service_id: s.id, max_age_days: None, radius_km: None, hair_length: String::new(), stylist_level: String::new(), include_starting_at: false, position: None, target_price: None };
            let e = market::evidence(c, &q, today, None)?;
            if let Some(st) = &e.evidence.stats {
                market_rows.push(MarketComparison { service_id: s.id, service_name: s.name.clone(), your_price: s.price, median: Some(st.median), q1: Some(st.q1), q3: Some(st.q3), n: st.n, quality: e.evidence.quality.clone() });
            }
        }
        Ok(Snapshot {
            inventory_value: inventory::products(c, true)?.iter().map(|p| p.value).sum(),
            products: products.len(),
            low_stock: inventory::reorder_list(c)?,
            upcoming: appointments::list(c, &now, &until, None, None)?.into_iter().filter(|a| a.status == "scheduled" || a.status == "checked_in").collect(),
            pricing_alerts: services.into_iter().filter(|s| s.status != "ok").collect(),
            market: market_rows,
        })
    })
}

/// Write rows prepared by the UI to a CSV file chosen in a save dialog.
#[tauri::command]
pub async fn export_csv(state: State<'_, AppState>, path: String, headers: Vec<String>, rows: Vec<Vec<String>>) -> AppResult<usize> {
    let p = state.output_path(&path, "csv")?;
    let h: Vec<&str> = headers.iter().map(String::as_str).collect();
    crate::csvio::write(&p, &h, rows)
}

// ---------------------------------------------------------------- market

#[tauri::command]
pub async fn competitors_list(state: State<'_, AppState>, include_archived: bool) -> AppResult<Vec<Competitor>> {
    state.read(|c| market::competitors(c, include_archived))
}
#[tauri::command]
pub async fn competitor_save(state: State<'_, AppState>, competitor: CompetitorInput) -> AppResult<i64> {
    state.tx(|c| market::save_competitor(c, &competitor, "manual", None, ""))
}
#[tauri::command]
pub async fn competitor_archive(state: State<'_, AppState>, id: i64, archived: bool) -> AppResult<()> {
    state.tx(|c| crate::db::core::set_archived(c, "competitors", "competitor", id, archived))
}
#[tauri::command]
pub async fn observations_list(state: State<'_, AppState>, service_id: Option<i64>) -> AppResult<Vec<Observation>> {
    state.read(|c| market::observations(c, service_id))
}
#[tauri::command]
pub async fn observation_save(state: State<'_, AppState>, observation: ObservationInput) -> AppResult<i64> {
    state.tx(|c| market::save_observation(c, &observation, "user_observed"))
}
#[tauri::command]
pub async fn observation_exclude(state: State<'_, AppState>, id: i64, excluded: bool) -> AppResult<()> {
    state.tx(|c| market::set_excluded(c, id, excluded))
}
#[tauri::command]
pub async fn observation_delete(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    state.tx(|c| market::delete_observation(c, id))
}
#[tauri::command]
pub async fn market_evidence(state: State<'_, AppState>, query: EvidenceQuery, today: String) -> AppResult<MarketEvidence> {
    let cpi = bls::cached_cpi(&state)?;
    state.read(|c| market::evidence(c, &query, &today, cpi.as_ref()))
}
#[tauri::command]
pub async fn observations_import_preview(state: State<'_, AppState>, path: String, mapping: ObservationMapping, default_service: Option<i64>) -> AppResult<Vec<ObsImportRow>> {
    let t = crate::csvio::read(&PathBuf::from(path))?;
    state.read(|c| market::preview_observations(c, &t, &mapping, default_service))
}
#[tauri::command]
pub async fn observations_import_commit(state: State<'_, AppState>, path: String, mapping: ObservationMapping, default_service: Option<i64>) -> AppResult<usize> {
    let t = crate::csvio::read(&PathBuf::from(path))?;
    state.tx(|c| market::import_observations(c, &t, &mapping, default_service))
}

#[derive(Serialize)]
pub struct DiscoverResult {
    pub found: usize,
    pub added: usize,
    pub updated: usize,
    pub attribution: String,
}

#[tauri::command]
pub async fn competitors_discover(state: State<'_, AppState>, radius_km: f64) -> AppResult<DiscoverResult> {
    let loc = state.read(|c| {
        let id = get_business(c)?.primary_location_id.ok_or_else(|| AppError::msg("Set a primary location first."))?;
        location(c, id)
    })?;
    let (lat, lon) = match (loc.latitude.as_deref().and_then(|v| v.parse::<f64>().ok()), loc.longitude.as_deref().and_then(|v| v.parse::<f64>().ok())) {
        (Some(a), Some(b)) => (a, b),
        _ => return Err(AppError::msg("Locate your business first (Settings → Locations → Locate) so nearby businesses can be found.")),
    };
    let found = overpass::nearby(&state, lat, lon, radius_km).await?;
    let attribution = crate::providers::info("overpass")?.attribution.unwrap_or_default().to_string();
    let (added, updated) = state.tx(|c| market::import_osm(c, &found, &attribution))?;
    Ok(DiscoverResult { found: found.len(), added, updated, attribution })
}

#[derive(Serialize)]
pub struct LocalContext {
    pub census: Vec<census::ContextFigure>,
    pub census_error: Option<String>,
    pub wages: Vec<(String, Vec<bls::SeriesPoint>)>,
    pub cpi: Vec<bls::SeriesPoint>,
    pub bls_error: Option<String>,
}

#[tauri::command]
pub async fn market_context(state: State<'_, AppState>) -> AppResult<LocalContext> {
    let loc = state.read(|c| {
        let id = get_business(c)?.primary_location_id.ok_or_else(|| AppError::msg("Set a primary location first."))?;
        location(c, id)
    })?;
    let (census, census_error) = match census::context(&state, &loc).await {
        Ok(v) => (v, None),
        Err(e) => (vec![], Some(e.to_string())),
    };
    let (wages, cpi, bls_error) = match (bls::wages(&state, loc.state_fips.as_deref()).await, bls::cpi(&state).await) {
        (Ok(w), Ok(c)) => (w, c, None),
        (Err(e), _) | (_, Err(e)) => (vec![], vec![], Some(e.to_string())),
    };
    Ok(LocalContext { census, census_error, wages, cpi, bls_error })
}

#[tauri::command]
pub async fn report_tax(state: State<'_, AppState>, filter: ReportFilter) -> AppResult<Vec<reports::TaxRow>> {
    state.read(|c| reports::tax_report(c, &filter))
}
