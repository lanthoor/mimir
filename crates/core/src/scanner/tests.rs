//! Tests for the directory scanner.

use std::fs;
use std::path::Path;

use crate::db::Library;
use crate::scanner::{hash_file, scan_root, upsert_folder, walk_audio_files, ScanJob};

fn touch(dir: &Path, name: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    fs::write(&p, b"").expect("touch");
    p
}

#[test]
fn walk_yields_audio_files_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    touch(root, "a.mp3");
    touch(root, "b.flac");
    let nested = root.join("Album");
    fs::create_dir(&nested).expect("mkdir");
    touch(&nested, "01 - Track.opus");
    touch(&nested, "cover.jpg");
    touch(root, "notes.txt");
    let deeper = nested.join("Disc 2");
    fs::create_dir(&deeper).expect("mkdir");
    touch(&deeper, "song.wav");
    touch(&deeper, "ignored.DS_Store");

    let mut found: Vec<_> = walk_audio_files(root)
        .map(|p| p.strip_prefix(root).unwrap().to_path_buf())
        .collect();
    found.sort();

    assert_eq!(
        found,
        vec![
            std::path::PathBuf::from("Album/01 - Track.opus"),
            std::path::PathBuf::from("Album/Disc 2/song.wav"),
            std::path::PathBuf::from("a.mp3"),
            std::path::PathBuf::from("b.flac"),
        ]
    );
}

#[test]
fn hash_file_is_deterministic_and_reports_metadata() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path().join("song.mp3");
    fs::write(&p, b"hello world").expect("write");

    let h1 = hash_file(&p).expect("hash 1");
    let h2 = hash_file(&p).expect("hash 2");

    // Same content → same blake3 hash.
    assert_eq!(h1.path_hash, h2.path_hash);
    assert_eq!(h1.path_hash.len(), 32);

    // Metadata matches `std::fs::metadata`.
    let meta = fs::metadata(&p).expect("metadata");
    assert_eq!(h1.size_bytes, meta.len().cast_signed());
    let expected_mtime_ns = i64::try_from(
        meta.modified()
            .expect("mtime")
            .duration_since(std::time::UNIX_EPOCH)
            .expect("since epoch")
            .as_nanos(),
    )
    .expect("mtime fits in i64");
    assert_eq!(h1.mtime_ns, expected_mtime_ns);
}

#[test]
fn hash_file_differs_for_different_content() {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = dir.path().join("a.mp3");
    let b = dir.path().join("b.mp3");
    fs::write(&a, b"abc").expect("write a");
    fs::write(&b, b"xyz").expect("write b");

    let ha = hash_file(&a).expect("hash a");
    let hb = hash_file(&b).expect("hash b");
    assert_ne!(ha.path_hash, hb.path_hash);
}

#[test]
fn upsert_folder_is_idempotent() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");

    let id1 = upsert_folder(&conn, "/music/A").expect("first");
    let id2 = upsert_folder(&conn, "/music/A").expect("second");
    assert_eq!(id1, id2, "same path must return same folder id");

    let id3 = upsert_folder(&conn, "/music/B").expect("third");
    assert_ne!(id1, id3, "different path must return a new id");

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM folder", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 2, "only two folders must be persisted");
}

#[test]
fn scan_root_emits_jobs_for_new_files_and_skips_known() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");

    let root = tempfile::tempdir().expect("tempdir");
    let new_file = touch(root.path(), "fresh.mp3");
    let known_file = touch(root.path(), "known.mp3");
    fs::write(&known_file, b"same content").expect("write known");

    // Pre-import `known_file` with its current hash so scan_root must skip it.
    let h = hash_file(&known_file).expect("hash known");
    conn.execute(
        "INSERT INTO folder (path, path_hash, active) VALUES (?1, ?2, 1)",
        rusqlite::params!["/dummy/folder", &vec![0u8; 32][..]],
    )
    .expect("insert folder");
    let folder_id: i64 = conn
        .query_row("SELECT id FROM folder LIMIT 1", [], |row| row.get(0))
        .expect("folder id");
    conn.execute(
        "INSERT INTO track (path, path_hash, mtime_ns, size_bytes, codec, folder_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            known_file.to_string_lossy(),
            &h.path_hash[..],
            h.mtime_ns,
            h.size_bytes,
            "mp3",
            folder_id,
        ],
    )
    .expect("insert known track");

    let (tx, rx) = std::sync::mpsc::channel::<ScanJob>();
    scan_root(&conn, root.path(), tx).expect("scan_root");
    // scan_root takes tx by value and drops it before returning, so the
    // channel is closed and recv will exit when drained.

    let jobs: Vec<ScanJob> = rx.into_iter().collect();
    // `known.mp3` is skipped; only `fresh.mp3` should be emitted.
    assert_eq!(jobs.len(), 1, "expected 1 new job, got {jobs:?}");
    assert_eq!(jobs[0].path, new_file);
}
