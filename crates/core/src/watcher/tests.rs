//! Tests for the file watcher.

use std::path::{Path, PathBuf};

use crate::watcher::{is_audio_path, to_ingest, EventKind, IngestEvent};

#[test]
fn ingest_event_carries_path_and_kind() {
    let ev = IngestEvent {
        path: PathBuf::from("/music/a.mp3"),
        kind: EventKind::Created,
    };
    assert_eq!(ev.path, PathBuf::from("/music/a.mp3"));
    assert_eq!(ev.kind, EventKind::Created);

    let renamed = IngestEvent {
        path: PathBuf::from("/music/b.flac"),
        kind: EventKind::Renamed {
            from: PathBuf::from("/music/a.flac"),
            to: PathBuf::from("/music/b.flac"),
        },
    };
    assert_eq!(
        renamed.kind,
        EventKind::Renamed {
            from: PathBuf::from("/music/a.flac"),
            to: PathBuf::from("/music/b.flac"),
        }
    );
}

#[test]
fn is_audio_path_accepts_supported_extensions() {
    for ext in ["mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "aiff", "alac"] {
        let path = format!("/x/song.{ext}");
        assert!(is_audio_path(Path::new(&path)), "{ext} should be audio");
    }
}

#[test]
fn is_audio_path_rejects_other_files() {
    for name in ["cover.jpg", "notes.txt", "AlbumArt.jpg", "track.mp3.bak", ".DS_Store", ""] {
        let p = Path::new(name);
        assert!(!is_audio_path(p), "{name} should not be audio");
    }
}

#[test]
fn is_audio_path_is_case_insensitive() {
    assert!(is_audio_path(Path::new("/x/Song.MP3")));
    assert!(is_audio_path(Path::new("/x/Song.Flac")));
}

#[test]
fn to_ingest_maps_create_and_modify() {
    use notify::event::{ModifyKind, EventKind as NEvent};

    let ev = notify::Event {
        kind: NEvent::Create(notify::event::CreateKind::File),
        paths: vec![PathBuf::from("/x/song.mp3")],
        ..notify::Event::default()
    };
    let mapped = to_ingest(&ev).expect("audio Create should map");
    assert_eq!(mapped.path, PathBuf::from("/x/song.mp3"));
    assert_eq!(mapped.kind, EventKind::Created);

    let ev = notify::Event {
        kind: NEvent::Modify(ModifyKind::Data(notify::event::DataChange::Any)),
        paths: vec![PathBuf::from("/x/song.flac")],
        ..notify::Event::default()
    };
    let mapped = to_ingest(&ev).expect("audio Modify should map");
    assert_eq!(mapped.kind, EventKind::Modified);
}

#[test]
fn to_ingest_drops_non_audio_paths() {
    use notify::event::EventKind as NEvent;

    let ev = notify::Event {
        kind: NEvent::Create(notify::event::CreateKind::File),
        paths: vec![PathBuf::from("/x/cover.jpg")],
        ..notify::Event::default()
    };
    assert!(to_ingest(&ev).is_none());
}

#[test]
fn to_ingest_handles_rename_with_from_and_to() {
    use notify::event::{EventKind as NEvent, RenameMode};

    let ev = notify::Event {
        kind: NEvent::Modify(notify::event::ModifyKind::Name(RenameMode::To)),
        paths: vec![PathBuf::from("/x/new.mp3")],
        ..notify::Event::default()
    };
    // The debouncer pairs a `From` and `To` event into one IngestEvent.
    let from = notify::Event {
        kind: NEvent::Modify(notify::event::ModifyKind::Name(RenameMode::From)),
        paths: vec![PathBuf::from("/x/old.mp3")],
        ..notify::Event::default()
    };
    let mapped = to_ingest_pair(&from, &ev).expect("rename pair should map");
    assert_eq!(mapped.path, PathBuf::from("/x/new.mp3"));
    assert_eq!(
        mapped.kind,
        EventKind::Renamed {
            from: PathBuf::from("/x/old.mp3"),
            to: PathBuf::from("/x/new.mp3"),
        }
    );
}
