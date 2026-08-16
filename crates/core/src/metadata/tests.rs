//! Tests for metadata extraction.

use std::fs;

use crate::metadata::{extract_tags, parse_filename, probe_file, HeuristicTags, Tags};

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
