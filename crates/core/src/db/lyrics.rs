//! Track lyrics storage.
//!
//! ponytail: only unsynced text is supported — LRC-style timing parsing
//! is deferred. Add a `synced` column reader when tier 4 (enrichment)
//! needs to surface a synced view.

use rusqlite::{params, Connection, OptionalExtension};

pub fn upsert_lyrics(
    conn: &Connection,
    track_id: i64,
    text: &str,
    language: &str,
    source: &str,
) -> rusqlite::Result<()> {
    mimir_telemetry::log(
        "DEBUG",
        "lyrics",
        &format!(
            "upsert_lyrics track_id={track_id} bytes={} lang={language} source={source}",
            text.len()
        ),
    );
    let n = conn.execute(
        "INSERT INTO lyrics (track_id, language, text, source) \
         VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(track_id, language) DO UPDATE SET \
            text = excluded.text, source = excluded.source",
        params![track_id, language, text, source],
    )?;
    mimir_telemetry::log(
        "INFO",
        "lyrics",
        &format!("upsert_lyrics track_id={track_id} rows={n}"),
    );
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsRow {
    pub text: String,
    pub language: String,
    pub source: String,
}

pub fn track_lyrics(
    conn: &Connection,
    track_id: i64,
) -> Result<Option<LyricsRow>, rusqlite::Error> {
    mimir_telemetry::log(
        "DEBUG",
        "lyrics",
        &format!("track_lyrics track_id={track_id}"),
    );
    let row: Option<(String, String, String)> = conn
        .query_row(
            "SELECT text, language, source FROM lyrics \
             WHERE track_id = ?1 \
             ORDER BY language \
             LIMIT 1",
            [track_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let out = row.map(|(t, l, s)| LyricsRow {
        text: t,
        language: l,
        source: s,
    });
    mimir_telemetry::log(
        "INFO",
        "lyrics",
        &format!(
            "track_lyrics track_id={track_id} present={}",
            out.is_some()
        ),
    );
    Ok(out)
}
