//! `artist` listing.

use rusqlite::Connection;
use serde::Serialize;

/// An artist row as returned by the read-side query layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtistRow {
    pub id: i64,
    pub name: String,
    pub sort_name: Option<String>,
    /// Total number of tracks filed under this artist's albums.
    pub track_count: i64,
}

/// Return every artist, sorted by `sort_name` (case-insensitive), with
/// `NULL`s last, each with the track count across their albums.
pub fn list_artists(conn: &Connection) -> Result<Vec<ArtistRow>, rusqlite::Error> {
    mimir_telemetry::log("DEBUG", "query", "list_artists");
    let mut stmt = conn.prepare(
        "SELECT ar.id, ar.name, ar.sort_name, COUNT(t.id) \
          FROM artist ar \
          LEFT JOIN album a ON a.album_artist_id = ar.id \
          LEFT JOIN track t ON t.album_id = a.id \
          GROUP BY ar.id \
          ORDER BY ar.sort_name COLLATE NOCASE ASC, ar.name COLLATE NOCASE ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ArtistRow {
                id: row.get(0)?,
                name: row.get(1)?,
                sort_name: row.get(2)?,
                track_count: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    mimir_telemetry::log(
        "INFO",
        "query",
        &format!("list_artists returned n={}", rows.len()),
    );
    Ok(rows)
}
