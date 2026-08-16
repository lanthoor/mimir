//! FTS5 search over track titles, albums, and artists.

use rusqlite::Connection;

use super::tracks::{row_to_track, TrackRow};

/// Return up to `limit` tracks matching `query` (FTS5 MATCH syntax). The
/// result is ordered by FTS rank, then track id.
///
/// Supports field operators (`title:`, `album:`, `artist:`), phrase quotes,
/// `OR` / `AND`, `-negation`, prefix `*` — see SQLite FTS5 docs.
pub fn search_tracks(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> Result<Vec<TrackRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.path, t.title, t.track_no, t.disc_no, t.duration_ms, t.codec, \
                a.id, a.title, ar.id, ar.name \
         FROM track_fts f \
         JOIN track t ON t.id = f.rowid \
         LEFT JOIN album a  ON a.id  = t.album_id \
         LEFT JOIN artist ar ON ar.id = a.album_artist_id \
         WHERE track_fts MATCH ?1 \
         ORDER BY rank, t.id \
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![query, limit], row_to_track)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
