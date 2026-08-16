//! Tests for the read-side query layer.

use rusqlite::Connection;

use crate::db::Library;
use crate::metadata::ingest;
use crate::query::{list_albums, list_tracks, AlbumRow, TrackRow};
use crate::scanner::{hash_file, ScanJob};

fn seed_track(root: &std::path::Path, conn: &Connection, rel: &str, title: &str) -> i64 {
    let p = root.join(rel);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    // Use distinct content so blake3 hashes differ; and sleep briefly so
    // mtime_ns is unique even on coarse-resolution filesystems.
    std::fs::write(&p, title.as_bytes()).expect("write");
    std::thread::sleep(std::time::Duration::from_millis(5));
    let folder_id =
        crate::scanner::upsert_folder(conn, p.parent().unwrap()).expect("folder");
    let file_hash = hash_file(&p).expect("hash");
    let id = ingest(
        conn,
        ScanJob {
            folder_id,
            path: p.clone(),
            file_hash,
        },
    )
    .expect("ingest");

    conn.execute(
        "UPDATE track SET title = ?1 WHERE path = ?2",
        rusqlite::params![title, p.to_string_lossy()],
    )
    .expect("title");

    id
}

#[test]
fn list_tracks_returns_all_with_pagination() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let root = tempfile::tempdir().expect("tempdir");

    seed_track(root.path(), &conn, "A/01 - x.mp3", "Song A");
    seed_track(root.path(), &conn, "B/02 - y.mp3", "Song B");
    seed_track(root.path(), &conn, "C/03 - z.mp3", "Song C");

    let all: Vec<TrackRow> = list_tracks(&conn, 100, 0).expect("list");
    assert_eq!(all.len(), 3);

    let page: Vec<TrackRow> = list_tracks(&conn, 1, 1).expect("page");
    assert_eq!(page.len(), 1);

    // Pagination must not yield duplicates across pages.
    let p1 = list_tracks(&conn, 2, 0).expect("p1");
    let p2 = list_tracks(&conn, 2, 2).expect("p2");
    let total = list_tracks(&conn, 100, 0).expect("all");
    let combined: std::collections::HashSet<_> =
        p1.iter().chain(p2.iter()).map(|t| t.id).collect();
    let total_ids: std::collections::HashSet<_> = total.iter().map(|t| t.id).collect();
    assert_eq!(combined, total_ids);
}

#[test]
fn list_albums_joins_artist_name() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let root = tempfile::tempdir().expect("tempdir");

    // Same artist, two albums.
    seed_track(root.path(), &conn, "Radiohead/OK Computer/01 - Airbag.mp3", "Airbag");
    seed_track(root.path(), &conn, "Radiohead/Kid A/01 - Everything.mp3", "Everything");

    let albums: Vec<AlbumRow> = list_albums(&conn, 100, 0).expect("list");
    assert_eq!(albums.len(), 2);

    let titles: std::collections::HashSet<_> =
        albums.iter().map(|a| a.title.clone()).collect();
    assert!(titles.contains("OK Computer"));
    assert!(titles.contains("Kid A"));

    // Every album has the artist joined.
    for a in &albums {
        assert_eq!(a.artist_name.as_deref(), Some("Radiohead"));
    }
}
