//! Idempotent artist + album upserts.

use rusqlite::{Connection, OptionalExtension};

/// Persist an artist row keyed by `name`. Returns the row id; repeated calls
/// with the same name return the same id.
pub fn upsert_artist(conn: &Connection, name: &str) -> rusqlite::Result<i64> {
    if let Some(id) = conn
        .query_row("SELECT id FROM artist WHERE name = ?1", [name], |row| {
            row.get::<_, i64>(0)
        })
        .optional()?
    {
        return Ok(id);
    }

    let sort_name = sort_name(name);
    conn.execute(
        "INSERT OR IGNORE INTO artist (name, sort_name) VALUES (?1, ?2)",
        rusqlite::params![name, &sort_name],
    )?;
    conn.query_row("SELECT id FROM artist WHERE name = ?1", [name], |row| {
        row.get::<_, i64>(0)
    })
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
        return Ok(id);
    }

    conn.execute(
        "INSERT OR IGNORE INTO album (title, album_artist_id, year) VALUES (?1, ?2, ?3)",
        rusqlite::params![title, album_artist_id, year],
    )?;
    conn.query_row(
        "SELECT id FROM album WHERE title = ?1 AND album_artist_id = ?2",
        rusqlite::params![title, album_artist_id],
        |row| row.get::<_, i64>(0),
    )
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
