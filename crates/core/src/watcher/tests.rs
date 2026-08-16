//! Tests for the file watcher.

use std::path::PathBuf;

use crate::watcher::{EventKind, IngestEvent};

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
