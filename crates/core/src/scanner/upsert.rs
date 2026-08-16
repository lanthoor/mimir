//! Idempotent folder upsert.

use std::path::Path;

use blake3::Hasher;
use rusqlite::{Connection, OptionalExtension};

/// Persist a folder row keyed by a blake3 hash of its canonical path.
/// Returns the row id; repeated calls with the same path return the same id.
pub fn upsert_folder(conn: &Connection, path: impl AsRef<Path>) -> rusqlite::Result<i64> {
    let path_str = path.as_ref().to_string_lossy().into_owned();
    let path_hash = folder_hash(&path_str);

    // Fast path: already present.
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM folder WHERE path = ?1",
            [&path_str],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
    {
        return Ok(id);
    }

    // Insert; if another connection raced us, fall back to the existing id.
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO folder (path, path_hash, active) VALUES (?1, ?2, 1)",
        rusqlite::params![&path_str, &path_hash],
    )?;
    let _ = inserted; // either 0 or 1; either way we look up below.

    conn.query_row(
        "SELECT id FROM folder WHERE path = ?1",
        [&path_str],
        |row| row.get::<_, i64>(0),
    )
}

/// blake3 hash of a folder path (used as a secondary lookup key).
fn folder_hash(path: &str) -> Vec<u8> {
    let mut h = Hasher::new();
    h.update(path.as_bytes());
    h.finalize().as_bytes().to_vec()
}
