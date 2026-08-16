//! Tests for the SQLite-backed library store.

use std::path::PathBuf;

use crate::db::Library;

#[test]
fn open_creates_missing_database_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().join("library.sqlite");

    assert!(!path.exists(), "precondition: db file should not exist");

    let lib = Library::open(&path).expect("Library::open should succeed");
    drop(lib);

    assert!(path.exists(), "Library::open must create the db file");
}
