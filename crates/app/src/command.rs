//! IPC commands invoked from the Svelte front-end.

#[cfg(feature = "tauri")]
use std::path::Path;

#[cfg(feature = "tauri")]
use mimir_audio::TransportCommand;
#[cfg(feature = "tauri")]
use mimir_core::query::{AlbumRow, GenreRow, TrackRow, YearRow};

#[cfg(feature = "tauri")]
use crate::error::AppError;
#[cfg(feature = "tauri")]
use crate::state::{AppState, LibraryStatus};

/// Open (or create) the library database at `path`.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_open(state: tauri::State<'_, AppState>, path: String) -> Result<(), AppError> {
    state.open_library(Path::new(&path))
}

/// Snapshot of the library state for the front-end.
///
/// The library is now opened implicitly by `AppState::new()` at the user's
/// default data location. The SPA calls this on startup to detect open
/// failures (a banner tells the user) and recover by re-opening.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_status(state: tauri::State<'_, AppState>) -> LibraryStatus {
    state.library_status()
}

/// Enqueue a folder for scanning. Returns the folder row id.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_add_folder(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<i64, AppError> {
    state.add_folder(Path::new(&path))
}

/// Full-text search across the library. Returns matching tracks.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_search(
    state: tauri::State<'_, AppState>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<TrackRow>, AppError> {
    state.search(&query, limit.unwrap_or(50))
}

/// Cover art for an album as `(mime_type, bytes)`. `None` when the album
/// has no embedded (or fetched) cover.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_album_cover(
    state: tauri::State<'_, AppState>,
    album_id: i64,
) -> Result<Option<(String, Vec<u8>)>, AppError> {
    state.album_cover(album_id)
}

/// Paged list of albums.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_list_albums(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<AlbumRow>, AppError> {
    state.list_albums(limit.unwrap_or(100), offset.unwrap_or(0))
}

/// Distinct genres in the library with track counts.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_list_genres(state: tauri::State<'_, AppState>) -> Result<Vec<GenreRow>, AppError> {
    state.list_genres()
}

/// Distinct years (from albums) with track counts.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_list_years(state: tauri::State<'_, AppState>) -> Result<Vec<YearRow>, AppError> {
    state.list_years()
}

/// Start (or replace) playback with the given track id.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn audio_play(state: tauri::State<'_, AppState>, track_id: i64) -> Result<(), AppError> {
    state.play_track(track_id, &TransportCommand::Play(track_id))
}

#[cfg(feature = "tauri")]
#[tauri::command]
pub fn audio_pause(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.send_transport(TransportCommand::Pause);
    Ok(())
}

#[cfg(feature = "tauri")]
#[tauri::command]
pub fn audio_resume(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.send_transport(TransportCommand::Resume);
    Ok(())
}

#[cfg(feature = "tauri")]
#[tauri::command]
pub fn audio_stop(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.send_transport(TransportCommand::Stop);
    Ok(())
}

#[cfg(feature = "tauri")]
#[tauri::command]
pub fn audio_next(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.send_transport(TransportCommand::Next);
    Ok(())
}

#[cfg(feature = "tauri")]
#[tauri::command]
pub fn audio_previous(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.send_transport(TransportCommand::Previous);
    Ok(())
}
