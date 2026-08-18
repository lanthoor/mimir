//! Tests for the SQLite-backed library store.

use std::path::PathBuf;

use crate::db::{
    album_cover, attach_album_cover, detach_album_cover, update_track, Library, TrackPatch,
    UpdateError,
};
use crate::metadata::{upsert_album, upsert_artist, CoverArt};

const EXPECTED_TABLES: &[&str] = &[
    "artist",
    "album",
    "track",
    "folder",
    "playlist",
    "playlist_track",
    "schema_version",
    "cover_art",
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

#[test]
fn fts5_matches_diacritics() {
    let lib = Library::in_memory().expect("in-memory Library");
    let conn = lib.conn().expect("conn");

    // Seed an artist + album + track with a diacritic in the title.
    conn.execute(
        "INSERT INTO artist (name, sort_name) VALUES ('Björk', 'Bjork')",
        [],
    )
    .expect("insert artist");
    let artist_id: i64 = conn
        .query_row("SELECT id FROM artist WHERE name = 'Björk'", [], |row| {
            row.get(0)
        })
        .expect("artist id");
    conn.execute(
        "INSERT INTO album (title, album_artist_id) VALUES ('Homogénic', ?1)",
        [artist_id],
    )
    .expect("insert album");
    let album_id: i64 = conn
        .query_row(
            "SELECT id FROM album WHERE title = 'Homogénic'",
            [],
            |row| row.get(0),
        )
        .expect("album id");
    conn.execute(
        "INSERT INTO track (path, path_hash, mtime_ns, size_bytes, codec, title, album_id) \
         VALUES ('/x.mp3', X'00', 0, 0, 'mp3', 'Jóga', ?1)",
        [album_id],
    )
    .expect("insert track");

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM track_fts WHERE track_fts MATCH ?1",
            ["Joga"],
            |row| row.get(0),
        )
        .expect("query");
    assert_eq!(count, 1, "diacritic-insensitive search must match Jóga");
}

fn make_album_with_artist(conn: &rusqlite::Connection, artist: &str, album: &str) -> i64 {
    let artist_id = upsert_artist(conn, artist).expect("artist");
    upsert_album(conn, album, artist_id, None).expect("album")
}

fn png_cover(seed: u8) -> CoverArt {
    // 1x1 white PNG (canonical 67-byte payload) repeated to add entropy so
    // each invocation produces a distinct `content_hash` when needed.
    let bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let mut v = bytes;
    v.push(seed);
    CoverArt {
        mime_type: "image/png".to_string(),
        data: v,
    }
}

#[test]
fn attach_and_fetch_album_cover_round_trips() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let album_id = make_album_with_artist(&conn, "Björk", "Homogénic");

    let cover = png_cover(1);
    let cover_id = attach_album_cover(&conn, album_id, &cover, "embedded").expect("attach");

    let row = album_cover(&conn, album_id)
        .expect("fetch")
        .expect("present");
    assert_eq!(row.mime_type, "image/png");
    assert_eq!(row.data, cover.data);

    let stored_id: i64 = conn
        .query_row(
            "SELECT cover_art_id FROM album WHERE id = ?1",
            [album_id],
            |row| row.get(0),
        )
        .expect("col");
    assert_eq!(stored_id, cover_id);
}

#[test]
fn attach_reuses_existing_cover_for_same_bytes() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let album1 = make_album_with_artist(&conn, "Björk", "Homogénic");
    let artist_id = upsert_artist(&conn, "compilation").expect("artist");
    let album2 = upsert_album(&conn, "Homogénic (rerelease)", artist_id, None).expect("album");

    let cover = png_cover(2);
    let id1 = attach_album_cover(&conn, album1, &cover, "embedded").expect("first");
    let id2 = attach_album_cover(&conn, album2, &cover, "embedded").expect("second");
    assert_eq!(id1, id2, "same bytes must dedupe to one row");

    let distinct = png_cover(3);
    let id3 = attach_album_cover(&conn, album1, &distinct, "embedded").expect("third");
    assert_ne!(id1, id3);
}

#[test]
fn detach_album_cover_clears_link_but_preserves_row() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let album_id = make_album_with_artist(&conn, "Björk", "Homogénic");
    let cover_id = attach_album_cover(&conn, album_id, &png_cover(4), "embedded").expect("attach");
    detach_album_cover(&conn, album_id).expect("detach");

    assert!(album_cover(&conn, album_id).expect("fetch").is_none());
    let row_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cover_art WHERE id = ?1",
            [cover_id],
            |row| row.get(0),
        )
        .expect("count");
    assert_eq!(row_count, 1, "shared cover row must survive detach");
}

fn insert_track(conn: &rusqlite::Connection, path: &str, hash: i64) -> i64 {
    conn.execute(
        "INSERT INTO track (path, path_hash, mtime_ns, size_bytes, codec, title) \
         VALUES (?1, ?2, 0, 0, 'mp3', 'orig-title')",
        rusqlite::params![path, hash],
    )
    .expect("track");
    conn.query_row("SELECT last_insert_rowid()", [], |row| row.get(0))
        .expect("id")
}

#[test]
fn update_track_writes_provided_fields() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let id = insert_track(&conn, "/t.mp3", 42);

    update_track(
        &conn,
        id,
        &TrackPatch {
            title: Some(Some("New Title".into())),
            genre: Some(Some("Indie".into())),
            track_no: Some(Some(7)),
            ..TrackPatch::default()
        },
    )
    .expect("update");

    let (title, genre, tno): (String, String, i64) = conn
        .query_row(
            "SELECT title, genre, track_no FROM track WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read");
    assert_eq!(title, "New Title");
    assert_eq!(genre, "Indie");
    assert_eq!(tno, 7);
}

#[test]
fn update_track_clears_when_inner_option_is_none() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let id = insert_track(&conn, "/t.mp3", 1);
    conn.execute("UPDATE track SET genre = 'X' WHERE id = ?1", [id])
        .expect("seed");

    update_track(
        &conn,
        id,
        &TrackPatch {
            genre: Some(None),
            ..TrackPatch::default()
        },
    )
    .expect("update");

    let cleared: Option<String> = conn
        .query_row("SELECT genre FROM track WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .expect("read");
    assert!(cleared.is_none(), "clear path must null the column");
}

#[test]
fn update_track_leaves_untouched_fields_alone() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let id = insert_track(&conn, "/t.mp3", 2);
    conn.execute("UPDATE track SET title = 'keep' WHERE id = ?1", [id])
        .expect("seed");

    update_track(
        &conn,
        id,
        &TrackPatch {
            genre: Some(Some("Rock".into())),
            ..TrackPatch::default()
        },
    )
    .expect("update");

    let title: Option<String> = conn
        .query_row("SELECT title FROM track WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .expect("read");
    assert_eq!(title.as_deref(), Some("keep"));
}

#[test]
fn update_track_unknown_id_returns_not_found() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let err = update_track(
        &conn,
        9_999_999,
        &TrackPatch {
            title: Some(Some("x".into())),
            ..TrackPatch::default()
        },
    )
    .expect_err("missing row");
    assert!(
        matches!(err, UpdateError::NotFound(9_999_999)),
        "got {err:?}"
    );
}

#[test]
fn update_track_empty_patch_is_noop() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let id = insert_track(&conn, "/t.mp3", 3);
    let result = update_track(&conn, id, &TrackPatch::default()).expect("noop");
    assert_eq!(result, id);
}
