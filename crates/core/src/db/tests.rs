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
