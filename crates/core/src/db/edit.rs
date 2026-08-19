//! Track tag mutations.
//!
//! All fields are optional — `None` means "leave the column unchanged".
//! `Some(Some(s))` writes `s`; `Some(None)` clears the column. Numeric
//! fields with invalid values are written verbatim (callers validate).
//!
//! ponytail: edits persist only in the DB, not the on-disk file. To
//! write tags back, lift this into a `lofty`-backed adapter and a
//! settings toggle; defer until v1 tag-editor lands.

use rusqlite::{params_from_iter, Connection};

/// Editable subset of a track row. Outer `Option` = "should we touch
/// this field?"; inner `Option` = "value or clear?".
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TrackPatch {
    pub title: Option<Option<String>>,
    pub genre: Option<Option<String>>,
    pub year: Option<Option<i32>>,
    pub track_no: Option<Option<i32>>,
    pub disc_no: Option<Option<i32>>,
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("track {0} not found")]
    NotFound(i64),
}

/// Apply `patch` to the track with id `track_id`. Returns the track id.
///
/// Empty patches return `Ok(track_id)` without touching the DB.
pub fn update_track(
    conn: &Connection,
    track_id: i64,
    patch: &TrackPatch,
) -> Result<i64, UpdateError> {
    let exists: bool = conn
        .query_row("SELECT 1 FROM track WHERE id = ?1", [track_id], |row| {
            row.get::<_, i64>(0)
        })
        .map(|_| true)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(false),
            other => Err(other),
        })?;
    if !exists {
        return Err(UpdateError::NotFound(track_id));
    }

    let mut sets: Vec<&'static str> = Vec::new();
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(t) = &patch.title {
        sets.push("title = ?");
        args.push(Box::new(t.clone()));
    }
    if let Some(g) = &patch.genre {
        sets.push("genre = ?");
        args.push(Box::new(g.clone()));
    }
    if let Some(y) = patch.year {
        sets.push("year = ?");
        args.push(Box::new(y));
    }
    if let Some(t) = patch.track_no {
        sets.push("track_no = ?");
        args.push(Box::new(t));
    }
    if let Some(d) = patch.disc_no {
        sets.push("disc_no = ?");
        args.push(Box::new(d));
    }
    if sets.is_empty() {
        return Ok(track_id);
    }

    let sql = format!("UPDATE track SET {} WHERE id = ?", sets.join(", "));
    args.push(Box::new(track_id));
    let binds: Vec<&dyn rusqlite::ToSql> = args.iter().map(std::convert::AsRef::as_ref).collect();
    conn.execute(&sql, params_from_iter(binds))?;
    Ok(track_id)
}
