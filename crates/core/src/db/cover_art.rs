//! Cover art storage in the library store.
//!
//! Cover bytes are stored inline in `cover_art` and deduped by blake3 so
//! `Various Artists` rereleases of the same image don't multiply the row
//! count. Albums reference their chosen cover via `album.cover_art_id`.
// ponytail: inline BLOB storage is fine for personal libraries (≤ a few
// hundred covers × a few MB each); if high-res scans cause DB bloat,
// move to a sidecar cache directory keyed by `content_hash` and store
// only the path in `cover_art.data`.

use rusqlite::{params, Connection, OptionalExtension};

use crate::metadata::CoverArt;

/// Persist `cover` and link it as the primary cover for `album_id`.
///
/// Idempotent on `content_hash`: re-ingesting the same bytes returns the
/// same `cover_art.id`. Re-using the same cover across multiple albums
/// attaches the existing row to the new `album_id`.
pub fn attach_album_cover(
    conn: &Connection,
    album_id: i64,
    cover: &CoverArt,
    source: &str,
) -> rusqlite::Result<i64> {
    mimir_telemetry::log(
        "DEBUG",
        "cover",
        &format!(
            "attach_album_cover start album_id={album_id} mime={} bytes={} source={source}",
            cover.mime_type,
            cover.data.len()
        ),
    );
    let hash = content_hash(&cover.data);
    let cover_id = upsert_cover(conn, &cover.mime_type, &cover.data, &hash, source)?;
    conn.execute(
        "UPDATE album SET cover_art_id = ?1 WHERE id = ?2",
        params![cover_id, album_id],
    )?;
    mimir_telemetry::log(
        "INFO",
        "cover",
        &format!("attach_album_cover ok album_id={album_id} cover_id={cover_id}"),
    );
    Ok(cover_id)
}

/// Remove the cover from `album_id` (does not delete the shared row).
pub fn detach_album_cover(conn: &Connection, album_id: i64) -> rusqlite::Result<()> {
    mimir_telemetry::log(
        "DEBUG",
        "cover",
        &format!("detach_album_cover album_id={album_id}"),
    );
    let n = conn.execute(
        "UPDATE album SET cover_art_id = NULL WHERE id = ?1",
        [album_id],
    )?;
    mimir_telemetry::log(
        "INFO",
        "cover",
        &format!("detach_album_cover updated rows={n} album_id={album_id}"),
    );
    Ok(())
}

/// Fetch the cover (mime + bytes) for `album_id`, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverRow {
    pub mime_type: String,
    pub data: Vec<u8>,
}

pub fn album_cover(conn: &Connection, album_id: i64) -> Result<Option<CoverRow>, rusqlite::Error> {
    mimir_telemetry::log(
        "DEBUG",
        "cover",
        &format!("album_cover lookup album_id={album_id}"),
    );
    let row: Option<(String, Vec<u8>)> = conn
        .query_row(
            "SELECT c.mime_type, c.data \
             FROM album a JOIN cover_art c ON c.id = a.cover_art_id \
             WHERE a.id = ?1",
            [album_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let out = row.map(|(m, d)| CoverRow {
        mime_type: m,
        data: d,
    });
    mimir_telemetry::log(
        "INFO",
        "cover",
        &format!("album_cover album_id={album_id} present={}", out.is_some()),
    );
    Ok(out)
}

fn upsert_cover(
    conn: &Connection,
    mime_type: &str,
    data: &[u8],
    hash: &[u8; 32],
    source: &str,
) -> rusqlite::Result<i64> {
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM cover_art WHERE content_hash = ?1",
            [hash.as_slice()],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
    {
        mimir_telemetry::log(
            "DEBUG",
            "cover",
            &format!(
                "upsert_cover hit id={id} mime={mime_type} bytes={}",
                data.len()
            ),
        );
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO cover_art (mime_type, data, content_hash, source) \
         VALUES (?1, ?2, ?3, ?4)",
        params![mime_type, data, hash.as_slice(), source],
    )?;
    let id = conn.query_row(
        "SELECT id FROM cover_art WHERE content_hash = ?1",
        [hash.as_slice()],
        |row| row.get::<_, i64>(0),
    )?;
    mimir_telemetry::log(
        "INFO",
        "cover",
        &format!(
            "upsert_cover inserted id={id} mime={mime_type} bytes={} source={source}",
            data.len()
        ),
    );
    Ok(id)
}

fn content_hash(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}
