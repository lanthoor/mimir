//! `artist` listing.

use rusqlite::Connection;
use serde::Serialize;

/// An artist row as returned by the read-side query layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtistRow {
    pub id: i64,
    pub name: String,
    pub sort_name: Option<String>,
}

/// Return every artist, sorted by `sort_name` (case-insensitive), with
/// `NULL`s last.
pub fn list_artists(conn: &Connection) -> Result<Vec<ArtistRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, sort_name FROM artist \
         ORDER BY sort_name COLLATE NOCASE ASC, name COLLATE NOCASE ASC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ArtistRow {
                id: row.get(0)?,
                name: row.get(1)?,
                sort_name: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}
