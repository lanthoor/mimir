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

/// Return up to `limit` artists starting at `offset`, sorted by
/// `sort_name` (case-insensitive, NULLs last), with each artist's track
/// count across their albums. `ar.id` is the final tiebreaker so
/// diacritic-tied pages never duplicate rows.
pub fn list_artists(
    conn: &Connection,
    limit: i64,
    offset: i64,
) -> Result<Vec<ArtistRow>, rusqlite::Error> {
    mimir_telemetry::log(
        "DEBUG",
        "query",
        &format!("list_artists limit={limit} offset={offset}"),
    );
    let mut stmt = conn.prepare(
        "SELECT ar.id, ar.name, ar.sort_name, COUNT(t.id) \
          FROM artist ar \
          LEFT JOIN album a ON a.album_artist_id = ar.id \
          LEFT JOIN track t ON t.album_id = a.id \
          GROUP BY ar.id \
          ORDER BY ar.sort_name COLLATE NOCASE ASC, \
                   ar.name COLLATE NOCASE ASC, \
                   ar.id \
          LIMIT ?1 OFFSET ?2",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![limit, offset], |row| {
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

/// Total number of artists. Sibling of `list_artists`.
pub fn count_artists(conn: &Connection) -> Result<i64, rusqlite::Error> {
    mimir_telemetry::log("DEBUG", "query", "count_artists");
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM artist", [], |row| row.get(0))?;
    mimir_telemetry::log("INFO", "query", &format!("count_artists ok n={n}"));
    Ok(n)
}
