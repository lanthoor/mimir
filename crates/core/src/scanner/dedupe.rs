//! End-to-end scan driver.
//!
//! Walks a root, hashes each audio file, upserts the folder row, and
//! forwards a `ScanJob` for every file whose `(path_hash, mtime_ns,
//! size_bytes)` triple is not already present in `track`.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use mimir_telemetry as telemetry;
use rusqlite::{Connection, OptionalExtension};
use thiserror::Error;

use super::hash::{hash_file, FileHash};
use super::upsert::upsert_folder;
use super::walk::walk_audio_files;

/// A file that needs to be ingested by the metadata extractor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanJob {
    pub folder_id: i64,
    pub path: PathBuf,
    pub file_hash: FileHash,
}

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("folder does not exist: {0}")]
    NotFound(PathBuf),
    #[error("path is not a directory: {0}")]
    NotADirectory(PathBuf),
}

/// Diagnostic summary returned by a successful `scan_root`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct ScanSummary {
    /// Files visited by the recursive walk.
    pub walked: u64,
    /// New `ScanJob`s emitted (not previously seen by `(path_hash, mtime_ns, size_bytes)`).
    pub sent: u64,
    /// Files that already had a matching track row.
    pub known: u64,
    /// Files that disappeared or were unreadable between walk and hash.
    pub hashed_fail: u64,
}

/// Walk `root`, upsert the folder row, and emit a `ScanJob` for every
/// audio file that does not already match an existing `track` row.
///
/// `tx` is taken by value so the channel is closed when this function
/// returns, letting the receiver's `recv()` exit on its own.
///
/// `ScanError::NotFound` / `NotADirectory` are returned up-front so the
/// front-end can show a clear diagnostic; an empty reachable directory
/// still succeeds with a zero-count `ScanSummary`.
#[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
pub fn scan_root(
    conn: &Connection,
    root: &Path,
    tx: Sender<ScanJob>,
) -> Result<ScanSummary, ScanError> {
    telemetry::log(
        "INFO",
        "scanner",
        &format!("scan_root start root={}", root.display()),
    );
    let metadata = match std::fs::metadata(root) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            telemetry::log(
                "WARN",
                "scanner",
                &format!("scan_root root not found: {}", root.display()),
            );
            return Err(ScanError::NotFound(root.to_path_buf()));
        }
        Err(e) => {
            telemetry::log(
                "ERROR",
                "scanner",
                &format!("scan_root metadata error on {}: {}", root.display(), e),
            );
            return Err(ScanError::NotFound(root.to_path_buf()));
        }
    };
    if !metadata.is_dir() {
        telemetry::log(
            "WARN",
            "scanner",
            &format!("scan_root not a directory: {}", root.display()),
        );
        return Err(ScanError::NotADirectory(root.to_path_buf()));
    }

    let folder_id = match upsert_folder(conn, root) {
        Ok(id) => {
            telemetry::log("DEBUG", "scanner", &format!("upsert_folder ok id={id}"));
            id
        }
        Err(e) => {
            telemetry::log(
                "ERROR",
                "scanner",
                &format!("upsert_folder failed for {}: {e}", root.display()),
            );
            return Err(ScanError::Sqlite(e));
        }
    };

    let mut walked = 0u64;
    let mut sent = 0u64;
    let mut known = 0u64;
    let mut hashed_fail = 0u64;
    for path in walk_audio_files(root) {
        walked += 1;
        let Ok(file_hash) = hash_file(&path) else {
            hashed_fail += 1;
            telemetry::log(
                "WARN",
                "scanner",
                &format!("hash_file failed for {}", path.display()),
            );
            continue;
        };

        if is_known(conn, &file_hash)? {
            known += 1;
            // The (content, mtime, size) triple matches a known track —
            // but the filesystem path may have changed (rename or move
            // since the last scan). Refresh `track.path` so the Folders
            // view + playback point at the live location.
            conn.execute(
                "UPDATE track SET path = ?1 \
                 WHERE path_hash = ?2 AND mtime_ns = ?3 AND size_bytes = ?4",
                rusqlite::params![
                    path.to_string_lossy().into_owned(),
                    &file_hash.path_hash[..],
                    file_hash.mtime_ns,
                    file_hash.size_bytes,
                ],
            )?;
            telemetry::log(
                "DEBUG",
                "scanner",
                &format!("path refreshed for {}", path.display()),
            );
            continue;
        }

        if tx
            .send(ScanJob {
                folder_id,
                path: path.clone(),
                file_hash,
            })
            .is_ok()
        {
            sent += 1;
        } else {
            telemetry::log(
                "ERROR",
                "scanner",
                &format!(
                    "ScanJob send failed (channel closed) path={}",
                    path.display()
                ),
            );
        }
    }

    telemetry::log(
        "INFO",
        "scanner",
        &format!(
            "scan_root done root={root} folder_id={folder_id} walked={walked} sent={sent} known={known} hash_fail={hashed_fail}",
            root = root.display()
        ),
    );
    if sent == 0 && hashed_fail == 0 && walked == 0 {
        telemetry::log(
            "WARN",
            "scanner",
            &format!(
                "scan_root empty root={} walked=0 — directory may be empty or contain no audio files",
                root.display()
            ),
        );
    }
    Ok(ScanSummary {
        walked,
        sent,
        known,
        hashed_fail,
    })
}

fn is_known(conn: &Connection, h: &FileHash) -> rusqlite::Result<bool> {
    let found: Option<i64> = conn
        .query_row(
            "SELECT id FROM track WHERE path_hash = ?1 AND mtime_ns = ?2 AND size_bytes = ?3 LIMIT 1",
            rusqlite::params![&h.path_hash[..], h.mtime_ns, h.size_bytes],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}
