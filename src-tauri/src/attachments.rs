//! Receipt and document attachments, copied into the workspace's attachments folder and named by
//! content hash (identical files are stored once). They are included in backups.

use crate::db::now_utc;
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const MAX_BYTES: u64 = 25 * 1024 * 1024;
const ALLOWED: &[&str] = &["pdf", "png", "jpg", "jpeg", "webp", "heic", "gif", "txt", "csv"];

#[derive(Serialize, Debug, Clone)]
pub struct Attachment {
    pub id: i64,
    pub file_name: String,
    pub size: i64,
    pub created_at: String,
}

pub fn add(conn: &Connection, dir: &Path, source: &Path) -> AppResult<Attachment> {
    let meta = std::fs::metadata(source)?;
    if !meta.is_file() {
        return Err(AppError::msg("Choose a file, not a folder."));
    }
    if meta.len() > MAX_BYTES {
        return Err(AppError::msg("Attachments are limited to 25 MB."));
    }
    let ext = source.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).unwrap_or_default();
    if !ALLOWED.contains(&ext.as_str()) {
        return Err(AppError::msg(format!("Attach a PDF, image or text file ({}).", ALLOWED.join(", "))));
    }
    let bytes = std::fs::read(source)?;
    let sha: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    let stored = format!("{sha}.{ext}");
    std::fs::create_dir_all(dir)?;
    let dest = dir.join(&stored);
    if !dest.exists() {
        let tmp = dir.join(format!(".{stored}.tmp"));
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(tmp, &dest)?;
    }
    let file_name = source.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(stored.clone());
    conn.execute(
        "INSERT INTO attachments (sha256, file_name, stored_name, size, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![sha, file_name, stored, bytes.len() as i64, now_utc()],
    )?;
    Ok(Attachment { id: conn.last_insert_rowid(), file_name, size: bytes.len() as i64, created_at: now_utc() })
}

pub fn path(conn: &Connection, dir: &Path, id: i64) -> AppResult<PathBuf> {
    let stored: String = conn
        .query_row("SELECT stored_name FROM attachments WHERE id = ?1", [id], |r| r.get(0))
        .optional()?
        .ok_or_else(|| AppError::NotFound("Attachment not found.".into()))?;
    let p = dir.join(stored);
    if !p.exists() {
        return Err(AppError::NotFound("The attachment file is missing from the workspace folder. Restore it from a backup.".into()));
    }
    Ok(p)
}
