//! Tests for the app shell.

use mimir_audio::{TransportCommand, TransportState};

use crate::AppError;
use crate::AppState;

#[test]
fn app_error_serializes_as_tagged_json() {
    let err = AppError::PathNotFound("/missing".into());
    let json = serde_json::to_value(&err).expect("serialize");
    assert_eq!(json["kind"], "PathNotFound");
    assert_eq!(json["message"], "/missing");
}

#[test]
fn open_library_creates_db_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("library.sqlite");
    let state = AppState::new();
    state.open_library(&path).expect("open");
    assert!(path.exists());
}

#[test]
fn search_before_open_returns_error() {
    let state = AppState::new();
    let err = state.search("foo", 10).expect_err("should error");
    assert!(matches!(err, AppError::Internal(_)));
}

#[test]
fn transport_commands_dispatch_in_order() {
    let state = AppState::new();
    state.send_transport(TransportCommand::Play(42));
    assert_eq!(state.transport().state, TransportState::Playing);
    assert_eq!(state.transport().queue.current(), Some(42));

    state.send_transport(TransportCommand::Pause);
    assert_eq!(state.transport().state, TransportState::Paused);

    state.send_transport(TransportCommand::Stop);
    assert_eq!(state.transport().state, TransportState::Stopped);
}

#[test]
fn add_folder_upserts_and_returns_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("library.sqlite");
    let music = dir.path().join("music");
    std::fs::create_dir_all(&music).expect("mkdir");
    std::fs::write(music.join("track.mp3"), b"fake").expect("write");

    let state = AppState::new();
    state.open_library(&db).expect("open");
    let id = state.add_folder(&music).expect("add folder");
    assert!(id > 0);
    // Note: the ScanJob is async; the worker thread may still be running.
    // The folder row is enough to verify the command.
    let rows: Vec<i64> = {
        let conn = state.library().expect("lib").conn().expect("conn");
        let rows = conn
            .prepare("SELECT id FROM folder")
            .expect("prep")
            .query_map([], |r| r.get::<_, i64>(0))
            .expect("query")
            .map(Result::unwrap)
            .collect();
        rows
    };
    assert_eq!(rows, vec![id]);
}
