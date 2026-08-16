//! Tests for the read-side query layer.

use rusqlite::Connection;

use crate::db::Library;
use crate::metadata::ingest;
use crate::query::{list_tracks, TrackRow};
use crate::scanner::{hash_file, ScanJob};

fn seed_track(conn: &Connection, path: &str, title: &str) -> i64 {
    let p = std::path::Path::new(path);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(p, b"junk").expect("write");
    let folder_id = crate::scanner::upsert_folder(conn, p.parent().unwrap()).expect("folder");
    let file_hash = hash_file(p).expect("hash");
    let id = ingest(
        conn,
        ScanJob {
            folder_id,
            path: p.to_path_buf(),
            file_hash,
        },
    )
    .expect("ingest");

    // Overwrite the title so we can verify it appears.
    conn.execute(
        "UPDATE track SET title = ?1 WHERE path = ?2",
        rusqlite::params![title, path],
    )
    .expect("title");

    id
}

#[test]
fn list_tracks_returns_all_with_pagination() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");

    seed_track(&conn, "/music/A/01 - x.mp3", "Song A");
    seed_track(&conn, "/music/B/02 - y.mp3", "Song B");
    seed_track(&conn, "/music/C/03 - z.mp3", "Song C");

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
