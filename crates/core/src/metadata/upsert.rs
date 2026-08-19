//! Idempotent artist + album upserts.

use rusqlite::{params_from_iter, Connection, OptionalExtension};

/// Persist an artist row keyed by `name`. Returns the row id; repeated calls
/// with the same name return the same id.
pub fn upsert_artist(conn: &Connection, name: &str) -> rusqlite::Result<i64> {
    if let Some(id) = conn
        .query_row("SELECT id FROM artist WHERE name = ?1", [name], |row| {
            row.get::<_, i64>(0)
        })
        .optional()?
    {
        mimir_telemetry::log(
            "DEBUG",
            "metadata",
            &format!("upsert_artist hit id={id} name={name}"),
        );
        return Ok(id);
    }

    let sort_name = sort_name(name);
    conn.execute(
        "INSERT OR IGNORE INTO artist (name, sort_name) VALUES (?1, ?2)",
        params_from_iter([name.to_string(), sort_name.clone()]),
    )?;
    let id = conn.query_row("SELECT id FROM artist WHERE name = ?1", [name], |row| {
        row.get::<_, i64>(0)
    })?;
    mimir_telemetry::log(
        "INFO",
        "metadata",
        &format!("upsert_artist inserted id={id} name={name} sort_name={sort_name}"),
    );
    Ok(id)
}

/// Persist an album row keyed by `(title, album_artist_id)`. Returns the
/// row id; repeated calls with the same triple return the same id.
pub fn upsert_album(
    conn: &Connection,
    title: &str,
    album_artist_id: i64,
    year: Option<u32>,
) -> rusqlite::Result<i64> {
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM album WHERE title = ?1 AND album_artist_id = ?2",
            rusqlite::params![title, album_artist_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
    {
        mimir_telemetry::log(
            "DEBUG",
            "metadata",
            &format!("upsert_album hit id={id} title={title} artist={album_artist_id}"),
        );
        return Ok(id);
    }

    conn.execute(
        "INSERT OR IGNORE INTO album (title, album_artist_id, year) VALUES (?1, ?2, ?3)",
        rusqlite::params![title, album_artist_id, year],
    )?;
    let id = conn.query_row(
        "SELECT id FROM album WHERE title = ?1 AND album_artist_id = ?2",
        rusqlite::params![title, album_artist_id],
        |row| row.get::<_, i64>(0),
    )?;
    mimir_telemetry::log(
        "INFO",
        "metadata",
        &format!("upsert_album inserted id={id} title={title} artist={album_artist_id} year={year:?}"),
    );
    Ok(id)
}

/// `Björk` → `Bjork`; preserves case folding but strips leading articles.
fn sort_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let stripped = match lower
        .strip_prefix("the ")
        .or_else(|| lower.strip_prefix("a "))
    {
        Some(s) => s,
        None => &lower,
    };
    stripped
        .chars()
        .map(|c| if c.is_ascii_alphabetic() { c } else { ' ' })
        .collect::<String>()
        .trim()
        .to_string()
}
