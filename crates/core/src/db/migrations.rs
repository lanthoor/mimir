//! Schema migrations.
//!
//! Each `Migration` is applied at most once. The runner records the highest
//! applied version in `schema_version` and skips anything at or below it.

use rusqlite::Connection;

/// A single ordered schema migration.
#[derive(Debug)]
pub(crate) struct Migration {
    pub version: i64,
    pub sql: &'static str,
}

/// All migrations, oldest first. New entries go at the end and must bump
/// the `version`.
pub(crate) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../../migrations/0001_base.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("../../migrations/0002_fts5_diacritics.sql"),
    },
    Migration {
        version: 3,
        sql: include_str!("../../migrations/0003_track_folder.sql"),
    },
    Migration {
        version: 4,
        sql: include_str!("../../migrations/0004_unknown_artist.sql"),
    },
    Migration {
        version: 5,
        sql: include_str!("../../migrations/0005_track_dedupe_unique.sql"),
    },
    Migration {
        version: 6,
        sql: include_str!("../../migrations/0006_cover_art.sql"),
    },
    Migration {
        version: 7,
        sql: include_str!("../../migrations/0007_track_genre.sql"),
    },
    Migration {
        version: 8,
        sql: include_str!("../../migrations/0008_fts_genre_from_track.sql"),
    },
    Migration {
        version: 9,
        sql: include_str!("../../migrations/0009_lyrics.sql"),
    },
];

const SCHEMA_VERSION_DDL: &str = "CREATE TABLE IF NOT EXISTS schema_version (\
    version INTEGER PRIMARY KEY, \
    applied_at INTEGER NOT NULL DEFAULT (unixepoch())\
)";

/// Ensure the `schema_version` table exists, then apply pending migrations.
pub(crate) fn ensure_and_apply(conn: &Connection) -> Result<i64, rusqlite::Error> {
    conn.execute_batch(SCHEMA_VERSION_DDL)?;
    let current = current_version(conn)?;
    apply(conn, current)
}

/// Apply any migrations whose version is greater than `current`.
fn apply(conn: &Connection, current: i64) -> Result<i64, rusqlite::Error> {
    let mut applied = current;
    for m in MIGRATIONS {
        if m.version <= applied {
            continue;
        }
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(m.sql)?;
        tx.execute(
            "INSERT INTO schema_version (version) VALUES (?1)",
            [m.version],
        )?;
        tx.commit()?;
        applied = m.version;
    }
    Ok(applied)
}

/// Read the currently applied schema version; `0` if none.
fn current_version(conn: &Connection) -> Result<i64, rusqlite::Error> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'schema_version'",
        [],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Ok(0);
    }
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |row| row.get(0),
    )
}
