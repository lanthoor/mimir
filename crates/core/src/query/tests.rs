//! Tests for the read-side query layer.

use rusqlite::Connection;

use crate::db::Library;
use crate::metadata::ingest;
use crate::query::{
    count_albums, count_artists, count_folder_files, count_genres, count_tracks,
    count_tracks_filtered, count_years, list_albums, list_artists, list_folder_files, list_genres,
    list_tracks, list_tracks_filtered, list_years, search_tracks, search_tracks_page, AlbumRow,
    ArtistRow, TrackFilter, TrackRow,
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

    // Count sibling must agree with the page-bounded total.
    assert_eq!(count_tracks(&conn).expect("count"), 3);
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

    // Count sibling must agree.
    assert_eq!(count_albums(&conn).expect("count"), 2);

    // Paginating at limit=1, offset=1 must give exactly the second album
    // in id order with no overlap with the first page.
    let p1: std::collections::HashSet<_> = list_albums(&conn, 1, 0)
        .expect("p1")
        .into_iter()
        .map(|a| a.id)
        .collect();
    let p2: std::collections::HashSet<_> = list_albums(&conn, 1, 1)
        .expect("p2")
        .into_iter()
        .map(|a| a.id)
        .collect();
    let all: std::collections::HashSet<_> = list_albums(&conn, 100, 0)
        .expect("all")
        .into_iter()
        .map(|a| a.id)
        .collect();
    let union: std::collections::HashSet<_> = p1.iter().chain(p2.iter()).copied().collect();
    assert_eq!(union, all);
    assert!(p1.is_disjoint(&p2));
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

    let artists: Vec<ArtistRow> = list_artists(&conn, 100, 0).expect("list");
    assert_eq!(artists.len(), 3, "expected 3 artists, got {artists:?}");

    let by_name: std::collections::HashMap<&str, i64> = artists
        .iter()
        .map(|a| (a.name.as_str(), a.track_count))
        .collect();

    // Sorted by sort_name (lowercase, diacritics stripped by upsert).
    let names: Vec<&str> = artists.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["Björk", "Múm", "Unknown Artist"],
        "expected sort order, got {names:?}"
    );

    // Real artists carry their album's single track; the seeded placeholder has none.
    assert_eq!(by_name.get("Björk"), Some(&1));
    assert_eq!(by_name.get("Múm"), Some(&1));
    assert_eq!(by_name.get("Unknown Artist"), Some(&0));
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

    let genres = list_genres(&conn, 100, 0).expect("list");
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

    let years = list_years(&conn, 100, 0).expect("list");
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

    // Count sibling must agree with the page-bounded total.
    let total = count_tracks_filtered(
        &conn,
        &TrackFilter {
            genre: Some("Electronic".into()),
            ..TrackFilter::default()
        },
    )
    .expect("count");
    assert_eq!(total, 3);
}

fn touch_dir_chain(root: &std::path::Path, segments: &[&str]) -> std::path::PathBuf {
    let p = segments
        .iter()
        .fold(root.to_path_buf(), |acc, s| acc.join(s));
    std::fs::create_dir_all(&p).expect("mkdir");
    p
}

#[test]
fn list_folders_includes_every_dir_and_only_audio_files() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    // Root has: a track under `pop`, a cover.jpg we must ignore, and
    // an empty `notes/` subdir that must still appear.
    let pop = touch_dir_chain(root, &["music", "pop"]);
    let mp3 = pop.join("hit.mp3");
    std::fs::write(&mp3, b"x").expect("write");
    std::fs::write(pop.join("cover.jpg"), b"\xff").expect("jpg");
    touch_dir_chain(root, &["music", "notes"]);
    // An unreadable / non-audio sibling under pop — must not appear.
    std::fs::write(pop.join("notes.txt"), b"hi").expect("notes");

    // Index the .mp3 with a real title so the row resolves. Only the
    // /music root is a watched root so pop/ shows up as a child, not
    // as its own root entry.
    let root_folder = crate::scanner::upsert_folder(&conn, touch_dir_chain(root, &["music"]))
        .expect("root folder");
    conn.execute(
        "INSERT INTO track (path, path_hash, mtime_ns, size_bytes, codec, title, folder_id) \
         VALUES (?1, RANDOMBLOB(16), 0, 0, 'mp3', 'Hit', ?2)",
        rusqlite::params![mp3.to_string_lossy(), root_folder],
    )
    .expect("track");

    let view = crate::query::list_folders(&conn).expect("folders");
    assert_eq!(view.root_children.len(), 1, "music/ root must appear");
    let music_node = &view.root_children[0];

    // Every directory shows up, including the empty `notes/` one.
    let child_names: std::collections::HashSet<_> = music_node
        .children
        .iter()
        .filter_map(|n| n.name.clone())
        .collect();
    assert!(child_names.contains("pop"));
    assert!(child_names.contains("notes"), "empty dir must appear");

    // pop/ contains the .mp3 (with title) and skips the .jpg + .txt.
    let pop_node = music_node
        .children
        .iter()
        .find(|n| n.name.as_deref() == Some("pop"))
        .expect("pop node");
    assert_eq!(pop_node.files.len(), 1, "only the .mp3 should be listed");
    let f = &pop_node.files[0];
    assert!(f.path.ends_with("hit.mp3"));
    assert_eq!(f.title.as_deref(), Some("Hit"));
    assert!(f.track_id.is_some());

    // notes/ is present and has no files.
    let notes_node = music_node
        .children
        .iter()
        .find(|n| n.name.as_deref() == Some("notes"))
        .expect("notes node");
    assert!(notes_node.files.is_empty());

    // Icon (flat) mode lists every directory (roots included now).
    assert!(view
        .flat
        .iter()
        .any(|n| n.path == mp3.parent().unwrap().to_string_lossy()));
    assert!(view
        .flat
        .iter()
        .any(|n| n.path == touch_dir_chain(root, &["music", "notes"]).to_string_lossy()));
}

#[test]
fn list_folders_skips_inactive_roots() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    let music = touch_dir_chain(root, &["music"]);
    let track = music.join("t.mp3");
    std::fs::write(&track, b"x").expect("write");
    crate::scanner::upsert_folder(&conn, track.parent().unwrap()).expect("track folder");

    let id = crate::scanner::upsert_folder(&conn, &music).expect("upsert");
    conn.execute("UPDATE folder SET active = 0 WHERE id = ?1", [id])
        .expect("deactivate");

    let view = crate::query::list_folders(&conn).expect("folders");
    assert!(
        view.root_children.is_empty(),
        "inactive roots must not surface"
    );
    assert!(view.flat.is_empty());
}

#[test]
fn list_artists_paginates_no_dupes_and_count_matches() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let root = tempfile::tempdir().expect("tempdir");

    // Three distinct artists + the seeded "Unknown Artist" placeholder.
    seed_track(root.path(), &conn, "Björk/Homogenic/01.mp3", "Hunter");
    seed_track(root.path(), &conn, "Múm/Finally/01.mp3", "We");
    seed_track(root.path(), &conn, "Aphex Twin/Selected/01.mp3", "Xtal");

    let p1: std::collections::HashSet<_> = list_artists(&conn, 2, 0)
        .expect("p1")
        .into_iter()
        .map(|a| a.id)
        .collect();
    let p2: std::collections::HashSet<_> = list_artists(&conn, 2, 2)
        .expect("p2")
        .into_iter()
        .map(|a| a.id)
        .collect();
    let all: std::collections::HashSet<_> = list_artists(&conn, 100, 0)
        .expect("all")
        .into_iter()
        .map(|a| a.id)
        .collect();

    assert_eq!(p1.len(), 2);
    assert!(p1.is_disjoint(&p2));
    let union: std::collections::HashSet<_> = p1.iter().chain(p2.iter()).copied().collect();
    assert_eq!(union, all);

    // Count sibling must agree.
    assert_eq!(
        count_artists(&conn).expect("count"),
        i64::try_from(all.len()).unwrap()
    );
}

#[test]
fn list_genres_paginates_no_dupes_and_count_matches() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let root = tempfile::tempdir().expect("tempdir");

    // Seed five distinct genres via direct inserts to keep the test cheap.
    for (i, name) in ["Electronic", "Pop", "Rock", "Jazz", "Classical"]
        .into_iter()
        .enumerate()
    {
        let i = i64::try_from(i).expect("test index fits i64");
        let path = format!("/t-{i}.mp3");
        conn.execute(
            "INSERT INTO track (path, path_hash, mtime_ns, size_bytes, codec, title, genre) \
             VALUES (?1, ?5, 0, 0, 'mp3', 't', ?2)",
            rusqlite::params![path, name, i, i, i + 100],
        )
        .expect("track");
        let _ = root;
    }

    let p1: Vec<_> = list_genres(&conn, 2, 0).expect("p1");
    let p2: Vec<_> = list_genres(&conn, 2, 2).expect("p2");
    let p3: Vec<_> = list_genres(&conn, 2, 4).expect("p3");
    let all: Vec<_> = list_genres(&conn, 100, 0).expect("all");
    assert_eq!(p1.len(), 2);
    assert_eq!(p2.len(), 2);
    assert_eq!(p3.len(), 1, "tail page holds the single leftover row");
    let names: std::collections::HashSet<_> = all.iter().map(|g| g.name.clone()).collect();
    assert_eq!(names.len(), 5);
    let union: std::collections::HashSet<_> = p1
        .iter()
        .chain(p2.iter())
        .chain(p3.iter())
        .map(|g| g.name.clone())
        .collect();
    assert_eq!(union.len(), 5, "no overlaps across pages");
    assert_eq!(union, names);

    // Count sibling must agree.
    assert_eq!(count_genres(&conn).expect("count"), 5);
}

#[test]
fn list_years_paginates_no_dupes_and_count_matches() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");

    let artist_id: i64 = conn
        .query_row(
            "INSERT INTO artist (name, sort_name) VALUES ('R', 'r') RETURNING id",
            [],
            |r| r.get(0),
        )
        .expect("artist");

    for (track_idx, (album_title, year)) in (0_i64..).zip([
        ("A", 1997_i32),
        ("B", 2000),
        ("C", 2003),
        ("D", 2006),
        ("E", 2009),
    ]) {
        conn.execute(
            "INSERT INTO album (title, album_artist_id, year) VALUES (?1, ?2, ?3)",
            rusqlite::params![album_title, artist_id, year],
        )
        .expect("album");
        let album_id: i64 = conn
            .query_row("SELECT last_insert_rowid()", [], |r| r.get(0))
            .expect("album id");
        conn.execute(
            "INSERT INTO track (path, path_hash, mtime_ns, size_bytes, codec, title, album_id) \
             VALUES (?1, ?3, 0, 0, 'mp3', 't', ?2)",
            rusqlite::params![format!("/{album_title}.mp3"), album_id, track_idx + 10],
        )
        .expect("track");
    }

    let p1: Vec<_> = list_years(&conn, 2, 0).expect("p1");
    let p2: Vec<_> = list_years(&conn, 2, 2).expect("p2");
    let p3: Vec<_> = list_years(&conn, 2, 4).expect("p3");
    let all: Vec<_> = list_years(&conn, 100, 0).expect("all");
    assert_eq!(all.len(), 5);
    assert_eq!(p1.len(), 2);
    assert_eq!(p2.len(), 2);
    assert_eq!(p3.len(), 1, "tail page holds the single leftover row");
    let mut years_seen: std::collections::HashSet<i32> = std::collections::HashSet::new();
    for y in p1.iter().chain(p2.iter()).chain(p3.iter()) {
        years_seen.insert(y.year);
    }
    let years_total: std::collections::HashSet<i32> = all.iter().map(|y| y.year).collect();
    assert_eq!(years_seen, years_total);

    // Count sibling must agree.
    assert_eq!(count_years(&conn).expect("count"), 5);
}

#[test]
fn search_tracks_page_total_matches_rows() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let root = tempfile::tempdir().expect("tempdir");

    seed_track(root.path(), &conn, "A/01.mp3", "Money");
    seed_track(root.path(), &conn, "B/02.mp3", "Time");
    seed_track(root.path(), &conn, "C/03.mp3", "Breathe");

    // FTS hit: total must equal the number of matching tracks.
    let page = search_tracks_page(&conn, "money", 50, 0).expect("page");
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.total, 1);

    let page = search_tracks_page(&conn, "time OR breathe", 50, 0).expect("page");
    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.total, 2);

    // Page slicing must not change total.
    let page = search_tracks_page(&conn, "time OR breathe", 1, 0).expect("page");
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.total, 2);

    // LIKE fallback path: total must match the row count of the LIKE query.
    let page = search_tracks_page(&conn, "m", 50, 0).expect("page");
    assert!(page.total >= 1, "LIKE fallback should find at least Money");
    assert_eq!(
        usize::try_from(page.total).expect("total fits usize"),
        page.rows.len(),
        "LIKE fallback single-page fetch: rows equals total"
    );
}

#[test]
fn list_folder_files_paginates_no_dupes_and_count_matches() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");
    let root = tempfile::tempdir().expect("tempdir");
    let dir = root.path().join("music");
    std::fs::create_dir_all(&dir).expect("mkdir");
    let folder_id = crate::scanner::upsert_folder(&conn, &dir).expect("folder");

    // Seed 55 tracks so paging (limit=20) crosses the boundary twice.
    for i in 0..55 {
        let path = dir.join(format!("t-{i:03}.mp3"));
        std::fs::write(&path, b"x").expect("write");
        conn.execute(
            "INSERT INTO track (path, path_hash, mtime_ns, size_bytes, codec, folder_id) \
             VALUES (?1, RANDOMBLOB(16), 0, 0, 'mp3', ?2)",
            rusqlite::params![path.to_string_lossy(), folder_id],
        )
        .expect("track");
    }

    let total = count_folder_files(&conn, folder_id).expect("count");
    assert_eq!(total, 55);

    let p1: std::collections::HashSet<_> = list_folder_files(&conn, folder_id, 20, 0)
        .expect("p1")
        .into_iter()
        .map(|f| f.path)
        .collect();
    let p2: std::collections::HashSet<_> = list_folder_files(&conn, folder_id, 20, 20)
        .expect("p2")
        .into_iter()
        .map(|f| f.path)
        .collect();
    let p3: std::collections::HashSet<_> = list_folder_files(&conn, folder_id, 20, 40)
        .expect("p3")
        .into_iter()
        .map(|f| f.path)
        .collect();
    assert_eq!(p1.len(), 20);
    assert_eq!(p2.len(), 20);
    assert_eq!(p3.len(), 15, "tail page must hold the 15 leftover rows");
    let union: std::collections::HashSet<_> = p1
        .iter()
        .chain(p2.iter())
        .chain(p3.iter())
        .cloned()
        .collect();
    assert_eq!(union.len(), 55, "no overlaps across pages");
}
