//! Tests for the read-side query layer.

use rusqlite::Connection;

use crate::db::Library;
use crate::metadata::ingest;
use crate::query::{
    list_albums, list_artists, list_genres, list_tracks, list_tracks_filtered, list_years,
    search_tracks, AlbumRow, ArtistRow, TrackFilter, TrackRow,
};
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
    let folder_id = crate::scanner::upsert_folder(conn, p.parent().unwrap()).expect("folder");
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
    let combined: std::collections::HashSet<_> = p1.iter().chain(p2.iter()).map(|t| t.id).collect();
    let total_ids: std::collections::HashSet<_> = total.iter().map(|t| t.id).collect();
    assert_eq!(combined, total_ids);
}

#[test]
fn list_albums_joins_artist_name() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let root = tempfile::tempdir().expect("tempdir");

    // Same artist, two albums.
    seed_track(
        root.path(),
        &conn,
        "Radiohead/OK Computer/01 - Airbag.mp3",
        "Airbag",
    );
    seed_track(
        root.path(),
        &conn,
        "Radiohead/Kid A/01 - Everything.mp3",
        "Everything",
    );

    let albums: Vec<AlbumRow> = list_albums(&conn, 100, 0).expect("list");
    assert_eq!(albums.len(), 2);

    let titles: std::collections::HashSet<_> = albums.iter().map(|a| a.title.clone()).collect();
    assert!(titles.contains("OK Computer"));
    assert!(titles.contains("Kid A"));

    // Every album has the artist joined.
    for a in &albums {
        assert_eq!(a.artist_name.as_deref(), Some("Radiohead"));
    }
}

#[test]
fn list_artists_is_sorted_by_sort_name() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let root = tempfile::tempdir().expect("tempdir");

    // Migration 0004 seeds "Unknown Artist" first; pick names that sort
    // before / after it.
    seed_track(
        root.path(),
        &conn,
        "Björk/Homogenic/01 - Hunter.mp3",
        "Hunter",
    );
    seed_track(root.path(), &conn, "Múm/Finally We Are/01 - We.mp3", "We");

    let artists: Vec<ArtistRow> = list_artists(&conn).expect("list");
    let names: Vec<&str> = artists.iter().map(|a| a.name.as_str()).collect();

    // Sorted by sort_name (lowercase, diacritics stripped by upsert).
    assert_eq!(
        names,
        vec!["Björk", "Múm", "Unknown Artist"],
        "expected sort order, got {names:?}"
    );
}

#[test]
fn search_tracks_matches_title_via_fts() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let root = tempfile::tempdir().expect("tempdir");

    seed_track(root.path(), &conn, "A/01 - x.mp3", "Money");
    seed_track(root.path(), &conn, "B/02 - y.mp3", "Time");
    seed_track(root.path(), &conn, "C/03 - z.mp3", "Breathe");

    let hits = search_tracks(&conn, "money", 50).expect("search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title.as_deref(), Some("Money"));

    let hits = search_tracks(&conn, "time OR breathe", 50).expect("search");
    assert_eq!(hits.len(), 2);
}

#[test]
fn search_tracks_is_di_acritic_insensitive() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let root = tempfile::tempdir().expect("tempdir");

    seed_track(root.path(), &conn, "A/01 - x.mp3", "Jóga");

    let hits = search_tracks(&conn, "joga", 50).expect("search");
    assert_eq!(hits.len(), 1, "diacritic-insensitive FTS must match Jóga");
}

#[test]
fn list_genres_groups_and_counts_by_genre() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");

    conn.execute(
        "INSERT INTO artist (name, sort_name) VALUES ('Björk', 'Bjork')",
        [],
    )
    .expect("artist");
    let artist_id: i64 = conn
        .query_row("SELECT id FROM artist WHERE name = 'Björk'", [], |row| {
            row.get(0)
        })
        .expect("artist_id");
    conn.execute(
        "INSERT INTO album (title, album_artist_id, year) VALUES ('Homogenic', ?1, 1997)",
        [artist_id],
    )
    .expect("album");
    let album_id: i64 = conn
        .query_row(
            "SELECT id FROM album WHERE id = last_insert_rowid()",
            [],
            |row| row.get(0),
        )
        .expect("album_id");
    for (i, (path, genre)) in [
        ("/a/1.mp3", Some("Electronic")),
        ("/a/2.mp3", Some("Electronic")),
        ("/a/3.mp3", Some("Pop")),
        ("/a/4.mp3", None),
    ]
    .into_iter()
    .enumerate()
    {
        let hash = i64::from(u32::try_from(i).expect("test index fits u32"));
        conn.execute(
            "INSERT INTO track (path, path_hash, mtime_ns, size_bytes, codec, title, genre, album_id) \
             VALUES (?1, ?4, 0, 0, 'mp3', 't', ?2, ?3)",
            rusqlite::params![path, genre, album_id, hash],
        )
        .expect("track");
    }

    let genres = list_genres(&conn).expect("list");
    assert_eq!(genres.len(), 2);
    assert_eq!(genres[0].name, "Electronic");
    assert_eq!(genres[0].track_count, 2);
    assert_eq!(genres[1].name, "Pop");
    assert_eq!(genres[1].track_count, 1);
}

#[test]
fn list_years_groups_and_counts_by_album_year() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");

    conn.execute(
        "INSERT INTO artist (name, sort_name) VALUES ('Radiohead', 'radiohead')",
        [],
    )
    .expect("artist");
    let artist_id: i64 = conn
        .query_row(
            "SELECT id FROM artist WHERE name = 'Radiohead'",
            [],
            |row| row.get(0),
        )
        .expect("artist_id");

    let mut track_idx: i64 = 0;
    for (album_title, year, count) in [
        ("OK Computer", 1997_i32, 2_i64),
        ("Kid A", 2000, 1),
        ("Unknown Year Album", -1, 0), // no year → excluded
    ] {
        conn.execute(
            "INSERT INTO album (title, album_artist_id, year) VALUES (?1, ?2, ?3)",
            rusqlite::params![
                album_title,
                artist_id,
                if year < 0 { None } else { Some(year) }
            ],
        )
        .expect("album");
        let album_id: i64 = conn
            .query_row("SELECT last_insert_rowid()", [], |row| row.get(0))
            .expect("album_id");
        for _ in 0..count {
            conn.execute(
                "INSERT INTO track (path, path_hash, mtime_ns, size_bytes, codec, title, album_id) \
                 VALUES (?1, ?3, 0, 0, 'mp3', 't', ?2)",
                rusqlite::params![
                    format!("/{album_title}-{track_idx}.mp3"),
                    album_id,
                    track_idx + 10,
                ],
            )
            .expect("track");
            track_idx += 1;
        }
    }

    let years = list_years(&conn).expect("list");
    let pairs: Vec<(i32, i64)> = years.iter().map(|y| (y.year, y.track_count)).collect();
    assert_eq!(pairs, vec![(1997, 2), (2000, 1)]);
}

fn seed_filter_env(conn: &Connection) {
    conn.execute(
        "INSERT INTO artist (name, sort_name) VALUES ('Björk', 'Bjork')",
        [],
    )
    .expect("artist");
    let artist_id: i64 = conn
        .query_row("SELECT id FROM artist WHERE name = 'Björk'", [], |row| {
            row.get(0)
        })
        .expect("artist_id");
    conn.execute(
        "INSERT INTO album (title, album_artist_id, year) VALUES ('Homogenic', ?1, 1997)",
        [artist_id],
    )
    .expect("album");
    let album_id: i64 = conn
        .query_row("SELECT last_insert_rowid()", [], |row| row.get(0))
        .expect("album_id");

    let album_other_id: i64 = {
        conn.execute(
            "INSERT INTO album (title, album_artist_id, year) VALUES ('Vespertine', ?1, 2001)",
            [artist_id],
        )
        .expect("album");
        conn.query_row("SELECT last_insert_rowid()", [], |row| row.get(0))
            .expect("album_id")
    };

    for (tidx, (path, genre, album_id_local)) in [
        ("/a/01.mp3", Some("Electronic"), album_id),
        ("/a/02.mp3", Some("Electronic"), album_id),
        ("/a/03.mp3", Some("Pop"), album_id),
        ("/a/04.mp3", None, album_id),
        ("/v/01.mp3", Some("Electronic"), album_other_id),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, t)| {
        (
            i64::from(u32::try_from(i).expect("test idx fits u32")) + 1,
            t,
        )
    }) {
        conn.execute(
            "INSERT INTO track (path, path_hash, mtime_ns, size_bytes, codec, title, genre, album_id) \
             VALUES (?1, ?4, 0, 0, 'mp3', 't', ?2, ?3)",
            rusqlite::params![path, genre, album_id_local, tidx],
        )
        .expect("track");
    }
}

#[test]
fn list_tracks_filtered_by_genre_only() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    seed_filter_env(&conn);

    let rows = list_tracks_filtered(
        &conn,
        &TrackFilter {
            genre: Some("Electronic".into()),
            ..TrackFilter::default()
        },
        100,
        0,
    )
    .expect("query");
    assert_eq!(rows.len(), 3);
    for r in &rows {
        assert_eq!(r.genre.as_deref(), Some("Electronic"));
    }
}

#[test]
fn list_tracks_filtered_by_year_only() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    seed_filter_env(&conn);

    let rows = list_tracks_filtered(
        &conn,
        &TrackFilter {
            year: Some(1997),
            ..TrackFilter::default()
        },
        100,
        0,
    )
    .expect("query");
    assert_eq!(rows.len(), 4);
    for r in &rows {
        assert_eq!(r.year, Some(1997));
    }
}

#[test]
fn list_tracks_filtered_combines_predicates() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    seed_filter_env(&conn);

    let rows = list_tracks_filtered(
        &conn,
        &TrackFilter {
            genre: Some("Electronic".into()),
            year: Some(1997),
            ..TrackFilter::default()
        },
        100,
        0,
    )
    .expect("query");
    assert_eq!(rows.len(), 2);
    for r in &rows {
        assert_eq!(r.genre.as_deref(), Some("Electronic"));
        assert_eq!(r.year, Some(1997));
    }
}

#[test]
fn list_tracks_filtered_empty_returns_all() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    seed_filter_env(&conn);

    let rows = list_tracks_filtered(&conn, &TrackFilter::default(), 100, 0).expect("query");
    assert_eq!(rows.len(), 5);
}

#[test]
fn list_tracks_filtered_pagination() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    seed_filter_env(&conn);

    let p1 = list_tracks_filtered(
        &conn,
        &TrackFilter {
            genre: Some("Electronic".into()),
            ..TrackFilter::default()
        },
        2,
        0,
    )
    .expect("p1");
    let p2 = list_tracks_filtered(
        &conn,
        &TrackFilter {
            genre: Some("Electronic".into()),
            ..TrackFilter::default()
        },
        2,
        2,
    )
    .expect("p2");
    assert_eq!(p1.len(), 2);
    assert_eq!(p2.len(), 1, "third page holds the leftover row");
    let ids: std::collections::HashSet<_> = p1.iter().chain(p2.iter()).map(|t| t.id).collect();
    assert_eq!(ids.len(), 3);
}
