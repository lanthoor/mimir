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

/// Add multiple folders in one call. Returns the list of folder row ids.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_add_folders(
    state: tauri::State<'_, AppState>,
    paths: Vec<String>,
) -> Result<Vec<i64>, AppError> {
    let mut ids = Vec::with_capacity(paths.len());
    for p in paths {
        ids.push(state.add_folder(Path::new(&p))?);
    }
    Ok(ids)
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

/// Tracks filtered by optional genre/year/artist/album facets.
///
/// All fields are optional. Empty/missing means "do not filter on that facet".
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_query_tracks(
    state: tauri::State<'_, AppState>,
    genre: Option<String>,
    year: Option<i32>,
    artist_id: Option<i64>,
    album_id: Option<i64>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<TrackRow>, AppError> {
    state.query_tracks_filtered(
        genre,
        year,
        artist_id,
        album_id,
        limit.unwrap_or(100),
        offset.unwrap_or(0),
    )
}

/// Paged list of tracks. Used for the Tracks view's default render so the
/// UI never asks FTS to match an empty query (which is a SQLite syntax error).
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_list_tracks(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<TrackRow>, AppError> {
    state.list_tracks(limit.unwrap_or(100), offset.unwrap_or(0))
}

/// Editable tags for a single track.
#[cfg(feature = "tauri")]
#[derive(Debug, Clone, serde::Serialize)]
pub struct EditableTrackFields {
    pub title: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub track_no: Option<i32>,
    pub disc_no: Option<i32>,
}

#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_get_editable_track(
    state: tauri::State<'_, AppState>,
    track_id: i64,
) -> Result<EditableTrackFields, AppError> {
    state.get_editable_track(track_id)
}

#[cfg(feature = "tauri")]
#[derive(Debug, serde::Deserialize)]
pub struct TrackPatchInput {
    pub title: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub track_no: Option<i32>,
    pub disc_no: Option<i32>,
    /// Field names to clear (set to NULL). Valid: "title","genre","year",
    /// "track_no","disc_no". Use this when you want to unset a column
    /// rather than replace it.
    pub clear: Vec<String>,
}

#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_update_track(
    state: tauri::State<'_, AppState>,
    track_id: i64,
    patch: TrackPatchInput,
) -> Result<(), AppError> {
    use mimir_core::db::TrackPatch;
    let mut t: Option<Option<String>> = None;
    let mut g: Option<Option<String>> = None;
    let mut y: Option<Option<i32>> = None;
    let mut tn: Option<Option<i32>> = None;
    let mut dn: Option<Option<i32>> = None;
    for f in &patch.clear {
        match f.as_str() {
            "title" => t = Some(None),
            "genre" => g = Some(None),
            "year" => y = Some(None),
            "track_no" => tn = Some(None),
            "disc_no" => dn = Some(None),
            _ => {}
        }
    }
    if patch.title.is_some() {
        t = Some(patch.title);
    }
    if patch.genre.is_some() {
        g = Some(patch.genre);
    }
    if patch.year.is_some() {
        y = Some(patch.year);
    }
    if patch.track_no.is_some() {
        tn = Some(patch.track_no);
    }
    if patch.disc_no.is_some() {
        dn = Some(patch.disc_no);
    }
    state.update_track(
        track_id,
        TrackPatch {
            title: t,
            genre: g,
            year: y,
            track_no: tn,
            disc_no: dn,
        },
    )
}

#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_clear_track_field(
    state: tauri::State<'_, AppState>,
    track_id: i64,
    field: String,
) -> Result<(), AppError> {
    let patch = build_clear_patch(&field);
    state.update_track(track_id, patch)
}

#[cfg(feature = "tauri")]
fn build_clear_patch(field: &str) -> mimir_core::db::TrackPatch {
    use mimir_core::db::TrackPatch;
    match field {
        "title" => TrackPatch {
            title: Some(None),
            ..TrackPatch::default()
        },
        "genre" => TrackPatch {
            genre: Some(None),
            ..TrackPatch::default()
        },
        "year" => TrackPatch {
            year: Some(None),
            ..TrackPatch::default()
        },
        "track_no" => TrackPatch {
            track_no: Some(None),
            ..TrackPatch::default()
        },
        "disc_no" => TrackPatch {
            disc_no: Some(None),
            ..TrackPatch::default()
        },
        _ => TrackPatch::default(),
    }
}

/// Lyrics for a track as `(text, language, source)`. `None` when absent.
#[cfg(feature = "tauri")]
#[derive(Debug, Clone, serde::Serialize)]
pub struct LyricsPayload {
    pub text: String,
    pub language: String,
    pub source: String,
}

#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_track_lyrics(
    state: tauri::State<'_, AppState>,
    track_id: i64,
) -> Result<Option<LyricsPayload>, AppError> {
    state.track_lyrics(track_id).map(|opt| {
        opt.map(|r| LyricsPayload {
            text: r.text,
            language: r.language,
            source: r.source,
        })
    })
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
