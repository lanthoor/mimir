//! Tests for the directory scanner.

use std::fs;
use std::path::Path;

use crate::scanner::{hash_file, walk_audio_files};

fn touch(dir: &Path, name: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    fs::write(&p, b"").expect("touch");
    p
}

#[test]
fn walk_yields_audio_files_only() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    touch(&root, "a.mp3");
    touch(&root, "b.flac");
    let nested = root.join("Album");
    fs::create_dir(&nested).expect("mkdir");
    touch(&nested, "01 - Track.opus");
    touch(&nested, "cover.jpg");
    touch(&root, "notes.txt");
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
    assert_eq!(h1.size_bytes, meta.len() as i64);
    assert_eq!(h1.mtime_ns, meta.modified().expect("mtime").duration_since(std::time::UNIX_EPOCH).expect("since epoch").as_nanos() as i64);
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
