//! Idempotent folder upsert.

use std::path::Path;

use blake3::Hasher;
use mimir_telemetry as telemetry;
use rusqlite::{Connection, OptionalExtension};

/// Persist a folder row keyed by a blake3 hash of its canonical path.
/// Returns the row id; repeated calls with the same path return the same id.
///
/// A re-add of a previously soft-deleted folder flips `active` back to
/// `1` so the Folders view shows it again without a row duplication.
pub fn upsert_folder(conn: &Connection, path: impl AsRef<Path>) -> rusqlite::Result<i64> {
    let path_str = path.as_ref().to_string_lossy().into_owned();
    let path_hash = folder_hash(&path_str);

    // Fast path: already present. If the row was soft-deleted, revive it.
    if let Some((id, active)) = conn
        .query_row(
            "SELECT id, active FROM folder WHERE path = ?1",
            [&path_str],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
    {
        if active == 0 {
            conn.execute("UPDATE folder SET active = 1 WHERE id = ?1", [id])?;
            telemetry::log(
                "INFO",
                "scanner",
                &format!("upsert_folder revive id={id} path={path_str}"),
            );
        } else {
            telemetry::log(
                "DEBUG",
                "scanner",
                &format!("upsert_folder hit id={id} path={path_str}"),
            );
        }
        return Ok(id);
    }

    // Insert; if another connection raced us, fall back to the existing id.
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO folder (path, path_hash, active) VALUES (?1, ?2, 1)",
        rusqlite::params![&path_str, &path_hash],
    )?;
    let id = conn.query_row(
        "SELECT id FROM folder WHERE path = ?1",
        [&path_str],
        |row| row.get::<_, i64>(0),
    )?;
    telemetry::log(
        "INFO",
        "scanner",
        &format!("upsert_folder inserted id={id} path={path_str} inserted={inserted}"),
    );
    Ok(id)
}

/// blake3 hash of a folder path (used as a secondary lookup key).
fn folder_hash(path: &str) -> Vec<u8> {
    let mut h = Hasher::new();
    h.update(path.as_bytes());
    h.finalize().as_bytes().to_vec()
}
