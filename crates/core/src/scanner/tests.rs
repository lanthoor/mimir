//! Tests for the directory scanner.

use std::fs;
use std::path::Path;

use crate::scanner::walk_audio_files;

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
