//! Business settings, locations, staff, audit log.

use crate::db::core::{self as q, AuditEntry, Business, Location, Staff};
use crate::error::AppResult;
use crate::workspace::AppState;
use tauri::State;

#[tauri::command]
pub async fn business_get(state: State<'_, AppState>) -> AppResult<Business> {
    state.read(q::get_business)
}

#[tauri::command]
pub async fn business_save(state: State<'_, AppState>, business: Business) -> AppResult<Business> {
    state.tx(|c| {
        q::save_business(c, &business)?;
        q::get_business(c)
    })
}

#[tauri::command]
pub async fn locations_list(state: State<'_, AppState>) -> AppResult<Vec<Location>> {
    state.read(q::locations)
}

#[tauri::command]
pub async fn location_save(state: State<'_, AppState>, location: Location) -> AppResult<Location> {
    state.tx(|c| {
        let id = q::save_location(c, &location)?;
        q::location(c, id)
    })
}

#[tauri::command]
pub async fn location_archive(state: State<'_, AppState>, id: i64, archived: bool) -> AppResult<()> {
    state.tx(|c| q::set_archived(c, "locations", "location", id, archived))
}

#[tauri::command]
pub async fn staff_list(state: State<'_, AppState>) -> AppResult<Vec<Staff>> {
    state.read(q::staff_list)
}

#[tauri::command]
pub async fn staff_save(state: State<'_, AppState>, staff: Staff) -> AppResult<i64> {
    state.tx(|c| q::save_staff(c, &staff))
}

#[tauri::command]
pub async fn staff_archive(state: State<'_, AppState>, id: i64, archived: bool) -> AppResult<()> {
    state.tx(|c| q::set_archived(c, "staff", "staff", id, archived))
}

#[tauri::command]
pub async fn audit_list(state: State<'_, AppState>, entity: Option<String>, before_id: Option<i64>, limit: Option<i64>) -> AppResult<Vec<AuditEntry>> {
    state.read(|c| q::audit_list(c, entity.as_deref(), before_id, limit.unwrap_or(100)))
}
