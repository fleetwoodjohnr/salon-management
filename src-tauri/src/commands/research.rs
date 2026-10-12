//! Market research: background runs, single-site checks, add-by-website, page details.

use crate::db::research::{self as store, PageRow, RunRow};
use crate::error::AppResult;
use crate::research::{self, Progress, ResearchParams, SiteLine};
use crate::workspace::AppState;
use tauri::{AppHandle, Emitter, Manager, State};

/// Start a search in the background; progress arrives as `research-progress` events.
#[tauri::command]
pub async fn research_start(app: AppHandle, state: State<'_, AppState>, params: ResearchParams) -> AppResult<i64> {
    let (run_id, ws_id, center) = research::start(&state, &params)?;
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        research::run(&state, run_id, &ws_id, params, center, &|p: &Progress| {
            let _ = app.emit("research-progress", p);
        })
        .await;
    });
    Ok(run_id)
}

#[tauri::command]
pub async fn research_cancel(state: State<'_, AppState>) -> AppResult<()> {
    state.research.cancel();
    Ok(())
}

/// The running (or most recent) search, for a page opened mid-run.
#[tauri::command]
pub async fn research_progress(state: State<'_, AppState>) -> AppResult<Option<Progress>> {
    Ok(state.research.snapshot_for(state.current_info().map(|w| w.id).as_deref()))
}

#[tauri::command]
pub async fn research_runs(state: State<'_, AppState>) -> AppResult<Vec<RunRow>> {
    state.read(|c| store::runs(c, 20))
}

#[tauri::command]
pub async fn research_competitor(state: State<'_, AppState>, id: i64) -> AppResult<SiteLine> {
    research::check_competitor(&state, id).await
}

#[tauri::command]
pub async fn competitor_add_website(state: State<'_, AppState>, url: String, name: Option<String>) -> AppResult<SiteLine> {
    research::add_website(&state, &url, name).await
}

#[tauri::command]
pub async fn competitor_pages(state: State<'_, AppState>, id: i64) -> AppResult<Vec<PageRow>> {
    state.read(|c| store::pages(c, id))
}

/// Match an observed price to one of your services by hand (or clear the match).
#[tauri::command]
pub async fn observation_match(state: State<'_, AppState>, id: i64, service_id: Option<i64>) -> AppResult<()> {
    state.tx(|c| store::set_match(c, id, service_id))
}
