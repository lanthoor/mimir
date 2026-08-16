//! `track` row listing + paging.

use rusqlite::Connection;

/// A track row as returned by the read-side query layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackRow {
    pub id: i64,
    pub path: String,
    pub title: Option<String>,
    pub track_no: Option<i32>,
    pub disc_no: Option<i32>,
    pub duration_ms: Option<i32>,
    pub codec: String,
    pub album_id: Option<i64>,
    pub album_title: Option<String>,
    pub artist_id: Option<i64>,
    pub artist_name: Option<String>,
}

/// Return up to `limit` tracks starting at `offset`, ordered by track id.
pub fn list_tracks(
    conn: &Connection,
    limit: i64,
    offset: i64,
) -> Result<Vec<TrackRow>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.path, t.title, t.track_no, t.disc_no, t.duration_ms, t.codec, \
                a.id, a.title, ar.id, ar.name \
         FROM track t \
         LEFT JOIN album a  ON a.id  = t.album_id \
         LEFT JOIN artist ar ON ar.id = a.album_artist_id \
         ORDER BY t.id \
         LIMIT ?1 OFFSET ?2",
    )?;

    let rows = stmt
        .query_map(rusqlite::params![limit, offset], row_to_track)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn row_to_track(row: &rusqlite::Row) -> rusqlite::Result<TrackRow> {
    Ok(TrackRow {
        id: row.get(0)?,
        path: row.get(1)?,
        title: row.get(2)?,
        track_no: row.get(3)?,
        disc_no: row.get(4)?,
        duration_ms: row.get(5)?,
        codec: row.get(6)?,
        album_id: row.get(7)?,
        album_title: row.get(8)?,
        artist_id: row.get(9)?,
        artist_name: row.get(10)?,
    })
}
