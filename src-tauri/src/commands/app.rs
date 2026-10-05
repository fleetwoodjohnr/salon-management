//! App status, workspaces and backups.

use crate::backup::{self, BackupInfo};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::workspace::{AppState, WorkspaceInfo, WorkspaceKind};
use serde::Serialize;
use std::path::PathBuf;
use tauri::State;

#[derive(Serialize)]
pub struct AppStatus {
    pub app_version: String,
    pub schema_version: i64,
    pub data_dir: String,
    pub workspaces: Vec<WorkspaceInfo>,
    pub current: Option<WorkspaceInfo>,
    pub open_error: Option<String>,
}

fn status(state: &AppState, open_error: Option<String>) -> AppResult<AppStatus> {
    Ok(AppStatus {
        app_version: env!("CARGO_PKG_VERSION").into(),
        schema_version: db::schema_version(),
        data_dir: state.root.to_string_lossy().into(),
        workspaces: state.list()?,
        current: state.current_info(),
        open_error,
    })
}

/// Called once at startup: reopens the last workspace if there is one.
#[tauri::command]
pub async fn app_status(state: State<'_, AppState>) -> AppResult<AppStatus> {
    let mut open_error = None;
    if state.current_info().is_none() {
        if let Some(id) = state.last_opened()? {
            if let Err(e) = state.open(&id) {
                open_error = Some(e.to_string());
            }
        }
    }
    status(&state, open_error)
}

#[tauri::command]
pub async fn workspace_create(state: State<'_, AppState>, name: String) -> AppResult<AppStatus> {
    let info = state.create(&name, WorkspaceKind::Business)?;
    state.open(&info.id)?;
    status(&state, None)
}

#[tauri::command]
pub async fn workspace_open(state: State<'_, AppState>, id: String) -> AppResult<AppStatus> {
    state.open(&id)?;
    status(&state, None)
}

#[tauri::command]
pub async fn workspace_close(state: State<'_, AppState>) -> AppResult<AppStatus> {
    state.close();
    status(&state, None)
}

#[tauri::command]
pub async fn workspace_rename(state: State<'_, AppState>, id: String, name: String) -> AppResult<AppStatus> {
    state.rename(&id, &name)?;
    status(&state, None)
}

#[tauri::command]
pub async fn workspace_delete(state: State<'_, AppState>, id: String, confirm_name: String) -> AppResult<AppStatus> {
    let ws = state.list()?.into_iter().find(|w| w.id == id).ok_or_else(|| AppError::NotFound("Workspace not found.".into()))?;
    if ws.name != confirm_name {
        return Err(AppError::invalid("confirm_name", "Type the workspace name exactly to confirm deletion."));
    }
    state.delete(&id)?;
    status(&state, None)
}

#[tauri::command]
pub async fn backup_list(state: State<'_, AppState>) -> AppResult<Vec<BackupInfo>> {
    let id = state.current_info().ok_or_else(|| AppError::NoWorkspace("No workspace is open.".into()))?.id;
    backup::list(&state.backups_dir(&id))
}

/// Manual backup. With `path` (from a save dialog) the zip goes there; otherwise into the
/// workspace's backup folder.
#[tauri::command]
pub async fn backup_create(state: State<'_, AppState>, path: Option<String>) -> AppResult<String> {
    state.with(|w| {
        let att = state.attachments_dir(&w.info.id);
        match path {
            Some(p) => {
                let p = state.output_path(&p, "zip")?;
                backup::create_at(&w.conn, &w.info, &att, &p, "manual")?;
                Ok(p.to_string_lossy().into())
            }
            None => Ok(backup::create(&w.conn, &w.info, &att, &state.backups_dir(&w.info.id), "manual")?
                .to_string_lossy()
                .into()),
        }
    })
}

#[tauri::command]
pub async fn backup_inspect(path: String) -> AppResult<backup::Manifest> {
    backup::read_manifest(&PathBuf::from(path))
}

#[tauri::command]
pub async fn backup_restore(state: State<'_, AppState>, path: String, replace_current: bool) -> AppResult<AppStatus> {
    state.restore(&PathBuf::from(path), replace_current)?;
    status(&state, None)
}

/// Create (or rebuild) the separate demo workspace and open it. Real workspaces are never touched.
#[tauri::command]
pub async fn demo_create(state: State<'_, AppState>) -> AppResult<AppStatus> {
    if let Some(old) = state.list()?.into_iter().find(|w| w.kind == WorkspaceKind::Demo) {
        state.delete(&old.id)?;
    }
    let info = state.create("Demo salon (sample data)", WorkspaceKind::Demo)?;
    state.open(&info.id)?;
    let today = chrono::Local::now().date_naive();
    state.tx(|c| crate::demo::seed(c, today, 6, 3))?;
    status(&state, None)
}

/// Portable export: every table as a CSV file in one zip (for spreadsheets or moving to other software).
#[tauri::command]
pub async fn export_all(state: State<'_, AppState>, path: String) -> AppResult<usize> {
    let p = state.output_path(&path, "zip")?;
    state.read(|c| crate::csvio::export_all(c, &p))
}
