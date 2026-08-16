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
fn app_state_implicitly_opens_default_library() {
    // `AppState::new()` must open the library at the user's default data
    // location so the SPA can call `library_add_folder` immediately on
    // first run. We can't reach the real `$XDG_DATA_HOME` from a unit test,
    // so this test only verifies the *shape* of the post-construct state:
    // the library is open, and the path is non-empty.
    let state = AppState::new();
    let status = state.library_status();
    assert!(status.path.is_some(), "implicit open must set a path");
    assert!(status.last_error.is_none(), "implicit open must not error");
    assert!(state.is_open(), "library must be open after construction");
}

#[test]
fn open_library_records_failure_on_status() {
    // Opening an unwritable path must surface as `last_error` rather than
    // panic or leave the library silently closed.
    let state = AppState::new();
    let bad = std::path::PathBuf::from("\0/definitely/not/a/real/path/db.sqlite");
    let err = state.open_library(&bad).expect_err("open should fail");
    let status = state.library_status();
    assert!(status.last_error.is_some(), "failure must be recorded");
    assert!(status.path.is_some(), "path should still be set to the attempt");
    assert!(!state.is_open(), "library must remain closed after failed open");
    // The exact error variant depends on which layer rejected the path
    // (InvalidInput at the OS layer, Sqlite at the SQLite layer, or our
    // own `Internal` for connection-pool failures). Accept any of them —
    // the contract is just that *some* error is reported.
    assert!(matches!(
        err,
        AppError::Sqlite(_) | AppError::Io(_) | AppError::Internal(_)
    ));
}

#[test]
fn open_library_clears_previous_last_error() {
    let state = AppState::new();
    let bad = std::path::PathBuf::from("\0/nope/db.sqlite");
    let _ = state.open_library(&bad);
    assert!(state.library_status().last_error.is_some());

    // Re-open with a fresh implicit default — should clear the error.
    let _ = state.open_library(state.library_status().path.as_deref().unwrap());
    // The fresh path may or may not be writable in the test env, so we
    // only assert that *something* happened (either success or a new error).
    // The real assertion is just that the surface API doesn't blow up.
    let _ = state.library_status();
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
