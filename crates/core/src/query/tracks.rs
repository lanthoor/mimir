//! `track` row listing + paging.

use rusqlite::Connection;
use serde::Serialize;

/// A track row as returned by the read-side query layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrackRow {
    pub id: i64,
    pub path: String,
    pub title: Option<String>,
    pub track_no: Option<i32>,
    pub disc_no: Option<i32>,
    pub duration_ms: Option<i32>,
    pub codec: String,
    pub genre: Option<String>,
    pub year: Option<i32>,
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
    mimir_telemetry::log(
        "DEBUG",
        "query",
        &format!("list_tracks limit={limit} offset={offset}"),
    );
    let mut stmt = conn.prepare(
        "SELECT t.id, t.path, t.title, t.track_no, t.disc_no, t.duration_ms, t.codec, \
                t.genre, a.year, \
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
    mimir_telemetry::log(
        "INFO",
        "query",
        &format!("list_tracks returned n={}", rows.len()),
    );
    Ok(rows)
}

pub(crate) fn row_to_track(row: &rusqlite::Row) -> rusqlite::Result<TrackRow> {
    Ok(TrackRow {
        id: row.get(0)?,
        path: row.get(1)?,
        title: row.get(2)?,
        track_no: row.get(3)?,
        disc_no: row.get(4)?,
        duration_ms: row.get(5)?,
        codec: row.get(6)?,
        genre: row.get(7)?,
        year: row.get(8)?,
        album_id: row.get(9)?,
        album_title: row.get(10)?,
        artist_id: row.get(11)?,
        artist_name: row.get(12)?,
    })
}

/// List distinct genres present in the library, sorted alphabetically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GenreRow {
    pub name: String,
    pub track_count: i64,
}

pub fn list_genres(conn: &Connection) -> Result<Vec<GenreRow>, rusqlite::Error> {
    mimir_telemetry::log("DEBUG", "query", "list_genres");
    let mut stmt = conn.prepare(
        "SELECT genre, COUNT(*) \
         FROM track \
         WHERE genre IS NOT NULL AND genre <> '' \
         GROUP BY genre \
         ORDER BY genre COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(GenreRow {
            name: row.get(0)?,
            track_count: row.get(1)?,
        })
    })?;
    let out: Vec<GenreRow> = rows.collect::<Result<_, _>>()?;
    mimir_telemetry::log(
        "INFO",
        "query",
        &format!("list_genres returned n={}", out.len()),
    );
    Ok(out)
}

/// List distinct years from albums, with track counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct YearRow {
    pub year: i32,
    pub track_count: i64,
}

pub fn list_years(conn: &Connection) -> Result<Vec<YearRow>, rusqlite::Error> {
    mimir_telemetry::log("DEBUG", "query", "list_years");
    let mut stmt = conn.prepare(
        "SELECT a.year, COUNT(t.id) \
         FROM album a JOIN track t ON t.album_id = a.id \
         WHERE a.year IS NOT NULL \
         GROUP BY a.year \
         ORDER BY a.year",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(YearRow {
            year: row.get(0)?,
            track_count: row.get(1)?,
        })
    })?;
    let out: Vec<YearRow> = rows.collect::<Result<_, _>>()?;
    mimir_telemetry::log(
        "INFO",
        "query",
        &format!("list_years returned n={}", out.len()),
    );
    Ok(out)
}
