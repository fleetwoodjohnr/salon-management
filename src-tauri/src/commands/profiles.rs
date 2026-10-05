//! Work profiles.

use crate::db::profiles::{self as q, ProfileVersionInfo, ProfileView};
use crate::db::core::set_archived;
use crate::domain::profile::{ProfileData, ProfileRates};
use crate::error::AppResult;
use crate::workspace::AppState;
use tauri::State;

#[tauri::command]
pub async fn profiles_list(state: State<'_, AppState>, include_archived: bool) -> AppResult<Vec<ProfileView>> {
    state.read(|c| q::list(c, include_archived))
}

#[tauri::command]
pub async fn profile_get(state: State<'_, AppState>, id: i64) -> AppResult<ProfileView> {
    state.read(|c| q::get(c, id))
}

#[tauri::command]
pub async fn profile_versions(state: State<'_, AppState>, id: i64) -> AppResult<Vec<ProfileVersionInfo>> {
    state.read(|c| q::versions(c, id))
}

/// Live preview of derived rates while editing (nothing is saved).
#[tauri::command]
pub async fn profile_preview(data: ProfileData) -> AppResult<ProfileRates> {
    data.rates()
}

#[tauri::command]
pub async fn profile_save(state: State<'_, AppState>, id: Option<i64>, name: String, data: ProfileData) -> AppResult<ProfileView> {
    state.tx(|c| {
        let id = q::save(c, id, &name, &data)?;
        q::get(c, id)
    })
}

#[tauri::command]
pub async fn profile_archive(state: State<'_, AppState>, id: i64, archived: bool) -> AppResult<()> {
    state.tx(|c| set_archived(c, "work_profiles", "work_profile", id, archived))
}
