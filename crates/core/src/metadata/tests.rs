//! Tests for metadata extraction.

use std::fs;

use crate::db::Library;
use crate::metadata::{
    extract_tags, ingest, parse_filename, probe_file, upsert_album, upsert_artist, HeuristicTags, Tags,
};
use crate::scanner::ScanJob;

fn minimal_mp3() -> Vec<u8> {
    // A minimal valid MPEG audio frame is enough for `lofty` to recognize
    // the format and report a codec. The first frame header:
    //   sync (0xFFE) | MPEG2 Layer3 | 128kbps | 44.1kHz | mono
    let mut bytes = vec![0xFF, 0xFB, 0x90, 0x00];
    // Pad to a few KB so lofty has something to scan.
    bytes.extend(std::iter::repeat_n(0u8, 4096));
    bytes
}

#[test]
fn probe_detects_mp3_codec() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path().join("song.mp3");
    fs::write(&p, minimal_mp3()).expect("write");

    let probe = probe_file(&p).expect("probe");
    assert_eq!(probe.codec, "mp3");
}

#[test]
fn probe_detects_flac_from_extension_when_no_magic() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path().join("song.flac");
    fs::write(&p, b"fLaC fake").expect("write");

    // lofty may or may not accept a truncated FLAC stream — but it must
    // *not* report the file as a different format.
    let probe = probe_file(&p).expect("probe");
    assert!(
        ["flac", "unknown"].contains(&probe.codec.as_str()),
        "got codec {}",
        probe.codec
    );
}

#[test]
fn probe_rejects_unknown_extension() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path().join("data.xyz");
    fs::write(&p, b"junk").expect("write");
    assert!(probe_file(&p).is_err());
}

#[test]
fn extract_tags_handles_missing_tag_gracefully() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path().join("silent.mp3");
    fs::write(&p, minimal_mp3()).expect("write");

    // No embedded tags — should not error, just return mostly-None.
    let tags = extract_tags(&p).expect("extract");
    assert!(matches!(tags, Tags { .. }));
}

#[test]
fn heuristic_parses_artist_album_track_title() {
    let path = std::path::Path::new("/music/Pink Floyd/Dark Side of the Moon/05 - Money.flac");
    let h = parse_filename(path).expect("heuristic");
    assert_eq!(
        h,
        HeuristicTags {
            artist: Some("Pink Floyd".into()),
            album: Some("Dark Side of the Moon".into()),
            track_no: Some(5),
            title: Some("Money".into()),
        }
    );
}

#[test]
fn heuristic_returns_none_for_unparseable() {
    let path = std::path::Path::new("/music/random.mp3");
    assert!(parse_filename(path).is_none());
}

#[test]
fn heuristic_handles_two_digit_track_no() {
    let path = std::path::Path::new("/music/Bon Iver/For Emma/12 - re: stacks.flac");
    let h = parse_filename(path).expect("heuristic");
    assert_eq!(h.track_no, Some(12));
}

#[test]
fn upsert_artist_is_idempotent() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");

    let a1 = upsert_artist(&conn, "Björk").expect("first");
    let a2 = upsert_artist(&conn, "Björk").expect("second");
    assert_eq!(a1, a2);

    let b = upsert_artist(&conn, "Múm").expect("different");
    assert_ne!(a1, b);

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM artist", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 2);
}

#[test]
fn upsert_album_is_idempotent_and_respects_album_artist() {
    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");

    let artist_id = upsert_artist(&conn, "Radiohead").expect("artist");
    println!("artist_id={artist_id}");
    let album1 = upsert_album(&conn, "OK Computer", artist_id, Some(1997)).expect("first");
    let album2 = upsert_album(&conn, "OK Computer", artist_id, Some(1997)).expect("second");
    assert_eq!(album1, album2);

    // Same title, different artist → different row.
    let other_artist = upsert_artist(&conn, "compilation").expect("artist2");
    let album3 = upsert_album(&conn, "OK Computer", other_artist, Some(1997)).expect("third");
    assert_ne!(album1, album3);

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM album", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 2);
}

#[test]
fn ingest_writes_artist_album_and_track_in_one_tx() {
    use std::fs;

    let lib = Library::in_memory().expect("in-memory");
    let conn = lib.conn().expect("conn");

    // Stand up a folder row first (we don't run scan_root here).
    let folder_id = crate::scanner::upsert_folder(&conn, "/music").expect("folder");

    // A real file with no tags so the heuristic fallback kicks in.
    let dir = tempfile::tempdir().expect("tempdir");
    let artist_dir = dir.path().join("Björk");
    let album_dir = artist_dir.join("Homogénic");
    fs::create_dir_all(&album_dir).expect("mkdir");
    let track = album_dir.join("05 - Hunter.mp3");
    fs::write(&track, b"fake bytes").expect("write");

    let file_hash = crate::scanner::hash_file(&track).expect("hash");
    let job = ScanJob {
        folder_id,
        path: track.clone(),
        file_hash,
    };

    let track_id = ingest(&conn, job).expect("ingest");

    // Artist + album + track rows exist.
    // (Migration 0004 seeds an "Unknown Artist" row; Björk is the second.)
    let artist_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM artist", [], |row| row.get(0))
        .expect("artist count");
    assert_eq!(artist_count, 2, "Unknown Artist (seed) + Björk");

    let bjork_id: i64 = conn
        .query_row("SELECT id FROM artist WHERE name = 'Björk'", [], |row| row.get(0))
        .expect("björk id");
    assert!(bjork_id > 0);

    let album_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM album", [], |row| row.get(0))
        .expect("album count");
    assert_eq!(album_count, 1);

    let track_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM track", [], |row| row.get(0))
        .expect("track count");
    assert_eq!(track_count, 1);
    assert!(track_id > 0);

    // FTS picks up the title.
    let hit: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM track_fts WHERE track_fts MATCH ?1",
            ["hunter"],
            |row| row.get(0),
        )
        .expect("fts");
    assert_eq!(hit, 1);
}
