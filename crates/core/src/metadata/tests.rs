//! Tests for metadata extraction.

use std::fs;

use crate::metadata::probe_file;

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
