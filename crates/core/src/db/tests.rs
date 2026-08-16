//! Tests for the SQLite-backed library store.

use std::path::PathBuf;

use crate::db::Library;

const EXPECTED_TABLES: &[&str] = &[
    "artist",
    "album",
    "track",
    "folder",
    "playlist",
    "playlist_track",
    "schema_version",
];

#[test]
fn open_creates_missing_database_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().join("library.sqlite");

    assert!(!path.exists(), "precondition: db file should not exist");

    let lib = Library::open(&path).expect("Library::open should succeed");
    drop(lib);

    assert!(path.exists(), "Library::open must create the db file");
}

#[test]
fn open_creates_expected_tables() {
    let lib = Library::in_memory().expect("in-memory Library");

    let conn = lib.conn().expect("conn");
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type IN ('table') ORDER BY name")
        .expect("prepare");

    let tables: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query_map")
        .map(Result::unwrap)
        .collect();

    for expected in EXPECTED_TABLES {
        assert!(
            tables.iter().any(|t| t == expected),
            "missing table `{expected}`; have {tables:?}"
        );
    }

    // track_fts is a virtual table; same query covers it.
    assert!(
        tables.iter().any(|t| t == "track_fts"),
        "missing virtual table `track_fts`; have {tables:?}"
    );
}

#[test]
fn reopen_does_not_reapply_migrations() {
    // Open once to apply migrations and record the schema version.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("library.sqlite");
    let _first = Library::open(&path).expect("first open");

    let version_after_first: i64 = {
        let conn = rusqlite::Connection::open(&path).expect("raw open");
        conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .expect("query version")
    };
    assert!(
        version_after_first >= 1,
        "first open should record version 1"
    );

    // Re-opening the same DB must not error (e.g. duplicate table) and
    // must not bump the recorded version.
    let _second = Library::open(&path).expect("second open");
    let version_after_second: i64 = {
        let conn = rusqlite::Connection::open(&path).expect("raw open");
        conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |row| row.get(0),
        )
        .expect("query version")
    };
    assert_eq!(
        version_after_first, version_after_second,
        "schema version must not change on reopen"
    );
}
