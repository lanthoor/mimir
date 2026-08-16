//! End-to-end scan driver.
//!
//! Walks a root, hashes each audio file, upserts the folder row, and
//! forwards a `ScanJob` for every file whose `(path_hash, mtime_ns,
//! size_bytes)` triple is not already present in `track`.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use rusqlite::{Connection, OptionalExtension};

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

/// Walk `root`, upsert the folder row, and emit a `ScanJob` for every
/// audio file that does not already match an existing `track` row.
///
/// A `track` row is considered "known" when its `(path_hash, mtime_ns,
/// size_bytes)` triple matches — same content + same mtime + same size. This
/// triple survives renames, so re-scans are cheap.
pub fn scan_root(
    conn: &Connection,
    root: &Path,
    tx: Sender<ScanJob>,
) -> rusqlite::Result<()> {
    let folder_id = upsert_folder(conn, root)?;
    for path in walk_audio_files(root) {
        let Ok(file_hash) = hash_file(&path) else {
            // File vanished or unreadable between walk and hash — drop.
            continue;
        };

        if is_known(conn, &file_hash)? {
            continue;
        }

        let _ = tx.send(ScanJob {
            folder_id,
            path,
            file_hash,
        });
    }
    Ok(())
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
