//! Independent business workspaces. Each one is a separate SQLite file plus an attachments folder,
//! so demo data and different businesses can never mix.
//!
//! Layout under the data root (platform app-data dir, or $SRM_DATA_DIR):
//!   workspaces.json                 registry
//!   workspaces/<id>/salon.db        database
//!   workspaces/<id>/attachments/    receipts and other files, named by content hash
//!   backups/<id>/*.zip              automatic, pre-migration and pre-restore backups

use crate::backup;
use crate::db;
use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceKind {
    Business,
    Demo,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
    pub kind: WorkspaceKind,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct Registry {
    workspaces: Vec<WorkspaceInfo>,
    last_opened: Option<String>,
}

pub struct OpenWorkspace {
    pub info: WorkspaceInfo,
    pub conn: Connection,
}

pub struct AppState {
    pub root: PathBuf,
    pub http: crate::providers::Http,
    current: Mutex<Option<OpenWorkspace>>,
    registry_lock: Mutex<()>,
}

impl AppState {
    pub fn new(root: PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(root.join("workspaces"))?;
        std::fs::create_dir_all(root.join("backups"))?;
        Ok(AppState { root, http: crate::providers::Http::new(), current: Mutex::new(None), registry_lock: Mutex::new(()) })
    }

    pub fn ws_dir(&self, id: &str) -> PathBuf {
        self.root.join("workspaces").join(id)
    }
    pub fn db_path(&self, id: &str) -> PathBuf {
        self.ws_dir(id).join("salon.db")
    }
    pub fn attachments_dir(&self, id: &str) -> PathBuf {
        self.ws_dir(id).join("attachments")
    }
    pub fn backups_dir(&self, id: &str) -> PathBuf {
        self.root.join("backups").join(id)
    }

    fn lock(&self) -> MutexGuard<'_, Option<OpenWorkspace>> {
        self.current.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn read_registry(&self) -> AppResult<Registry> {
        let p = self.root.join("workspaces.json");
        if !p.exists() {
            return Ok(Registry::default());
        }
        Ok(serde_json::from_str(&std::fs::read_to_string(p)?)?)
    }

    fn write_registry(&self, r: &Registry) -> AppResult<()> {
        let p = self.root.join("workspaces.json");
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(r)?)?;
        std::fs::rename(tmp, p)?;
        Ok(())
    }

    fn update_registry(&self, f: impl FnOnce(&mut Registry) -> AppResult<()>) -> AppResult<()> {
        let _g = self.registry_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut r = self.read_registry()?;
        f(&mut r)?;
        self.write_registry(&r)
    }

    pub fn list(&self) -> AppResult<Vec<WorkspaceInfo>> {
        Ok(self.read_registry()?.workspaces)
    }

    pub fn current_info(&self) -> Option<WorkspaceInfo> {
        self.lock().as_ref().map(|w| w.info.clone())
    }

    pub fn last_opened(&self) -> AppResult<Option<String>> {
        Ok(self.read_registry()?.last_opened)
    }

    pub fn create(&self, name: &str, kind: WorkspaceKind) -> AppResult<WorkspaceInfo> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::invalid("name", "Enter a workspace name."));
        }
        let info = WorkspaceInfo {
            id: format!("ws-{}", chrono::Utc::now().format("%Y%m%d%H%M%S%3f")),
            name: name.to_string(),
            kind,
            created_at: db::now_utc(),
        };
        std::fs::create_dir_all(self.attachments_dir(&info.id))?;
        self.update_registry(|r| {
            r.workspaces.push(info.clone());
            Ok(())
        })?;
        Ok(info)
    }

    /// Register an already-populated workspace directory (used by restore-as-new).
    pub fn register(&self, info: WorkspaceInfo) -> AppResult<()> {
        self.update_registry(|r| {
            r.workspaces.push(info);
            Ok(())
        })
    }

    pub fn rename(&self, id: &str, name: &str) -> AppResult<()> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::invalid("name", "Enter a workspace name."));
        }
        self.update_registry(|r| {
            let w = r.workspaces.iter_mut().find(|w| w.id == id).ok_or_else(|| AppError::NotFound("Workspace not found.".into()))?;
            w.name = name.clone();
            Ok(())
        })?;
        if let Some(open) = self.lock().as_mut() {
            if open.info.id == id {
                open.info.name = name;
            }
        }
        Ok(())
    }

    /// Open a workspace (closing any other), migrating it if needed. A backup is taken before
    /// any migration and once a day automatically.
    pub fn open(&self, id: &str) -> AppResult<WorkspaceInfo> {
        let info = self
            .list()?
            .into_iter()
            .find(|w| w.id == id)
            .ok_or_else(|| AppError::NotFound("Workspace not found.".into()))?;
        let mut guard = self.lock();
        *guard = None; // close the previous connection first
        std::fs::create_dir_all(self.attachments_dir(id))?;
        let backups = self.backups_dir(id);
        let attachments = self.attachments_dir(id);
        let conn = db::open(&self.db_path(id), |conn, from| {
            backup::create(conn, &info, &attachments, &backups, &format!("pre-migration-v{from}")).map(|_| ())
        })?;
        backup::auto_backup_if_due(&conn, &info, &attachments, &backups)?;
        *guard = Some(OpenWorkspace { info: info.clone(), conn });
        drop(guard);
        self.update_registry(|r| {
            r.last_opened = Some(id.to_string());
            Ok(())
        })?;
        Ok(info)
    }

    pub fn close(&self) {
        *self.lock() = None;
    }

    /// Delete a workspace's live data. A final backup is kept in the backups folder.
    pub fn delete(&self, id: &str) -> AppResult<()> {
        let info = self.list()?.into_iter().find(|w| w.id == id).ok_or_else(|| AppError::NotFound("Workspace not found.".into()))?;
        {
            let mut guard = self.lock();
            if guard.as_ref().is_some_and(|o| o.info.id == id) {
                *guard = None;
            }
        }
        if self.db_path(id).exists() {
            let conn = Connection::open(self.db_path(id))?;
            backup::create(&conn, &info, &self.attachments_dir(id), &self.backups_dir(id), "pre-delete")?;
        }
        self.update_registry(|r| {
            r.workspaces.retain(|w| w.id != id);
            if r.last_opened.as_deref() == Some(id) {
                r.last_opened = None;
            }
            Ok(())
        })?;
        let dir = self.ws_dir(id);
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
        Ok(())
    }

    /// Restore a verified backup, either as a new workspace (non-destructive, default) or by
    /// replacing the open workspace. Replacing first takes a pre-restore backup and keeps the old
    /// folder beside the new one as `<id>.pre-restore-<time>`.
    pub fn restore(&self, zip: &Path, replace_current: bool) -> AppResult<WorkspaceInfo> {
        let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S%3f").to_string();
        let scratch = self.root.join(format!(".restore-{ts}"));
        let manifest = match backup::verify_and_extract(zip, &scratch) {
            Ok(m) => m,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&scratch);
                return Err(e);
            }
        };
        if replace_current {
            let cur = self
                .current_info()
                .ok_or_else(|| AppError::NoWorkspace("Open the workspace you want to replace first.".into()))?;
            self.with(|w| {
                backup::create(&w.conn, &w.info, &self.attachments_dir(&cur.id), &self.backups_dir(&cur.id), "pre-restore")
            })?;
            self.close();
            let dir = self.ws_dir(&cur.id);
            std::fs::rename(&dir, self.root.join("workspaces").join(format!("{}.pre-restore-{ts}", cur.id)))?;
            std::fs::rename(&scratch, &dir)?;
            self.open(&cur.id)
        } else {
            let info = WorkspaceInfo {
                id: format!("ws-{ts}"),
                name: format!("{} (restored {})", manifest.workspace.name, &manifest.created_at[..10]),
                kind: manifest.workspace.kind,
                created_at: db::now_utc(),
            };
            std::fs::rename(&scratch, self.ws_dir(&info.id))?;
            self.register(info.clone())?;
            self.open(&info.id)
        }
    }

    /// Run `f` with the open workspace's connection.
    pub fn with<T>(&self, f: impl FnOnce(&mut OpenWorkspace) -> AppResult<T>) -> AppResult<T> {
        let mut guard = self.lock();
        let open = guard
            .as_mut()
            .ok_or_else(|| AppError::NoWorkspace("No workspace is open. Choose or create one first.".into()))?;
        f(open)
    }

    pub fn read<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        self.with(|w| f(&w.conn))
    }

    /// Run `f` inside a transaction; commits on Ok, rolls back on Err.
    pub fn tx<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        self.with(|w| {
            let tx = w.conn.transaction()?;
            let out = f(&tx)?;
            tx.commit()?;
            Ok(out)
        })
    }

    pub fn current_attachments_dir(&self) -> AppResult<PathBuf> {
        self.with(|w| Ok(self.attachments_dir(&w.info.id)))
    }
}

impl AppState {
    /// Check a path chosen in a save dialog before writing to it: right file type, an existing
    /// folder, and never inside the app's own data folder (so a bad request can't overwrite a
    /// database or backup).
    pub fn output_path(&self, path: &str, ext: &str) -> AppResult<PathBuf> {
        let p = PathBuf::from(path);
        if p.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()) != Some(ext.to_string()) {
            return Err(AppError::msg(format!("Choose a .{ext} file name.")));
        }
        let parent = p.parent().filter(|d| d.is_dir()).ok_or_else(|| AppError::msg("That folder doesn't exist."))?;
        let parent = parent.canonicalize()?;
        let root = self.root.canonicalize().unwrap_or_else(|_| self.root.clone());
        if parent.starts_with(&root) {
            return Err(AppError::msg("Save exports somewhere outside the app's data folder."));
        }
        Ok(parent.join(p.file_name().unwrap()))
    }
}

/// Resolve the data root: $SRM_DATA_DIR wins (tests, portable installs), else the platform dir.
pub fn data_root(platform_dir: &Path) -> PathBuf {
    std::env::var_os("SRM_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| platform_dir.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_paths_are_checked() {
        let base = std::env::temp_dir().join(format!("srm-ws-{}", std::process::id()));
        let root = base.join("data");
        let out = base.join("exports");
        std::fs::create_dir_all(&out).unwrap();
        let st = AppState::new(root.clone()).unwrap();
        assert!(st.output_path(out.join("a.csv").to_str().unwrap(), "csv").is_ok());
        assert!(st.output_path(out.join("a.txt").to_str().unwrap(), "csv").is_err());
        assert!(st.output_path(root.join("workspaces/x.csv").to_str().unwrap(), "csv").is_err());
        assert!(st.output_path(base.join("missing/a.csv").to_str().unwrap(), "csv").is_err());
        let _ = std::fs::remove_dir_all(base);
    }
}
