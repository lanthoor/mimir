//! Optional-facet filter for the read-side listing.
//!
//! `None` means "do not filter on this facet". Empty-string search means
//! "no text predicate". Tracks are returned ordered by id (stable paging).

use rusqlite::{params_from_iter, Connection, ToSql};

use super::tracks::{row_to_track, TrackRow};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TrackFilter {
    /// Genre to match exactly (case-sensitive: genres are usually already
    /// normalized by tag extractors).
    pub genre: Option<String>,
    /// Album year to match.
    pub year: Option<i32>,
    /// Artist id.
    pub artist_id: Option<i64>,
    /// Album id.
    pub album_id: Option<i64>,
}

pub fn list_tracks_filtered(
    conn: &Connection,
    filter: &TrackFilter,
    limit: i64,
    offset: i64,
) -> Result<Vec<TrackRow>, rusqlite::Error> {
    mimir_telemetry::log(
        "INFO",
        "query",
        &format!("list_tracks_filtered filter={filter:?} limit={limit} offset={offset}"),
    );
    let mut sql = String::from(
        "SELECT t.id, t.path, t.title, t.track_no, t.disc_no, t.duration_ms, t.codec, \
                t.genre, a.year, \
                a.id, a.title, ar.id, ar.name \
         FROM track t \
         LEFT JOIN album a  ON a.id  = t.album_id \
         LEFT JOIN artist ar ON ar.id = a.album_artist_id \
         WHERE 1 = 1",
    );
    let mut binds: Vec<Box<dyn ToSql>> = Vec::new();
    if let Some(g) = &filter.genre {
        sql.push_str(" AND t.genre = ?");
        binds.push(Box::new(g.clone()));
    }
    if let Some(y) = filter.year {
        sql.push_str(" AND a.year = ?");
        binds.push(Box::new(y));
    }
    if let Some(aid) = filter.artist_id {
        sql.push_str(" AND ar.id = ?");
        binds.push(Box::new(aid));
    }
    if let Some(abid) = filter.album_id {
        sql.push_str(" AND a.id = ?");
        binds.push(Box::new(abid));
    }
    sql.push_str(" ORDER BY t.id LIMIT ? OFFSET ?");
    binds.push(Box::new(limit));
    binds.push(Box::new(offset));

    let mut stmt = conn.prepare(&sql)?;
    let binds_ref: Vec<&dyn ToSql> = binds.iter().map(std::convert::AsRef::as_ref).collect();
    let rows = stmt.query_map(params_from_iter(binds_ref), row_to_track)?;
    let out: Vec<TrackRow> = rows.collect::<Result<_, _>>()?;
    mimir_telemetry::log(
        "INFO",
        "query",
        &format!("list_tracks_filtered returned n={}", out.len()),
    );
    Ok(out)
}
