//! `album` listing with joined artist.

use rusqlite::Connection;
use serde::Serialize;

/// An album row as returned by the read-side query layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AlbumRow {
    pub id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub artist_id: Option<i64>,
    pub artist_name: Option<String>,
    pub track_count: i64,
}

/// Return up to `limit` albums starting at `offset`, ordered by album id.
pub fn list_albums(
    conn: &Connection,
    limit: i64,
    offset: i64,
) -> Result<Vec<AlbumRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.title, a.year, ar.id, ar.name, \
                (SELECT COUNT(*) FROM track t WHERE t.album_id = a.id) \
         FROM album a \
         LEFT JOIN artist ar ON ar.id = a.album_artist_id \
         ORDER BY a.id \
         LIMIT ?1 OFFSET ?2",
    )?;

    let rows = stmt
        .query_map(rusqlite::params![limit, offset], row_to_album)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn row_to_album(row: &rusqlite::Row) -> rusqlite::Result<AlbumRow> {
    Ok(AlbumRow {
        id: row.get(0)?,
        title: row.get(1)?,
        year: row.get(2)?,
        artist_id: row.get(3)?,
        artist_name: row.get(4)?,
        track_count: row.get(5)?,
    })
}
