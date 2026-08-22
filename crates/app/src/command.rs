//! IPC commands invoked from the Svelte front-end.

#[cfg(feature = "tauri")]
use std::path::Path;

#[cfg(feature = "tauri")]
use mimir_audio::TransportCommand;
#[cfg(feature = "tauri")]
use mimir_core::query::{AlbumRow, FolderView, GenreRow, TrackRow, YearRow};
#[cfg(feature = "tauri")]
use mimir_core::rusqlite;

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

/// Folders view payload. `path` is the watched root's absolute path;
/// the front-end uses it to render the row + to confirm a remove dialog.
/// `file_count` is the number of indexed audio files in this directory
/// subtree (recursive) — purely informational for the row list.
#[cfg(feature = "tauri")]
#[derive(Debug, Clone, serde::Serialize)]
pub struct FolderRow {
    pub id: i64,
    pub path: String,
    pub file_count: i64,
}

/// Returns every active watched folder with its rolled-up track count.
/// Used by the Folders view to render the row list before the
/// per-directory tree is requested.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_list_folders(state: tauri::State<'_, AppState>) -> Result<Vec<FolderRow>, AppError> {
    state.list_folders()
}

/// Folder tree (icon-mode flat list + tree-mode hierarchy). Empty
/// folders are pruned by the query layer.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_folder_tree(state: tauri::State<'_, AppState>) -> Result<FolderView, AppError> {
    state.folder_tree()
}

/// Stop watching a folder. Soft-delete: the row is kept so future
/// rescans / re-adds work, but it disappears from the Folders view.
/// Indexed tracks are left in place so the user's Tracks / Albums /
/// Artists views remain intact.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_remove_folder(
    state: tauri::State<'_, AppState>,
    folder_id: i64,
) -> Result<(), AppError> {
    state.remove_folder(folder_id)
}

/// Update the filesystem path of a watched folder. Used when the user
/// renamed the directory on disk between runs — keeps the `path` column
/// in sync so the Folders view shows the new label and future queries
/// match the live FS state.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_rename_folder(
    state: tauri::State<'_, AppState>,
    folder_id: i64,
    new_path: String,
) -> Result<(), AppError> {
    state.rename_folder(folder_id, &new_path)
}

/// Rename a subdirectory on disk + rewrite `track.path` rows under it.
/// `current_path` is the full current path; `new_name` is a single path
/// segment (no separators).
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_rename_subdir(
    state: tauri::State<'_, AppState>,
    current_path: String,
    new_name: String,
) -> Result<(), AppError> {
    state.rename_subdir(&current_path, &new_name)
}

/// Reveal a file or directory in the platform's file manager.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_reveal_in_file_manager(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), AppError> {
    state.reveal_in_file_manager(&path)
}

/// Enqueue a folder for scanning. Returns the folder row id plus a
/// summary so the UI can tell the user "no audio files found" etc.
#[cfg(feature = "tauri")]
#[derive(Debug, serde::Serialize, Clone)]
pub struct AddFolderResult {
    pub folder_id: i64,
    pub summary: mimir_core::scanner::ScanSummary,
}

#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_add_folder(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<AddFolderResult, AppError> {
    let (id, summary) = state.add_folder(Path::new(&path))?;
    Ok(AddFolderResult {
        folder_id: id,
        summary,
    })
}

/// Add multiple folders in one call. Returns one `AddFolderResult` per path
/// (in the same order) so the UI can surface per-folder outcomes.
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_add_folders(
    state: tauri::State<'_, AppState>,
    paths: Vec<String>,
) -> Result<Vec<AddFolderResult>, AppError> {
    let mut out = Vec::with_capacity(paths.len());
    for p in paths {
        let (id, summary) = state.add_folder(Path::new(&p))?;
        out.push(AddFolderResult {
            folder_id: id,
            summary,
        });
    }
    Ok(out)
}

#[cfg(feature = "tauri")]
impl From<(i64, mimir_core::scanner::ScanSummary)> for AddFolderResult {
    fn from((folder_id, summary): (i64, mimir_core::scanner::ScanSummary)) -> Self {
        Self { folder_id, summary }
    }
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
/// UI never asks `FTS` to match an empty query (which is a `SQLite` syntax error).
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
    /// Field names to clear (set to NULL). Valid: `"title"`,`"genre"`,`"year"`,
    /// `"track_no"`,`"disc_no"`. Use this when you want to unset a column
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
    #[cfg(feature = "output")]
    state.send_player(mimir_audio::PlayerCommand::Pause);
    Ok(())
}

#[cfg(feature = "tauri")]
#[tauri::command]
pub fn audio_resume(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.send_transport(TransportCommand::Resume);
    #[cfg(feature = "output")]
    state.send_player(mimir_audio::PlayerCommand::Resume);
    Ok(())
}

#[cfg(feature = "tauri")]
#[tauri::command]
pub fn audio_stop(state: tauri::State<'_, AppState>) -> Result<(), AppError> {
    state.send_transport(TransportCommand::Stop);
    #[cfg(feature = "output")]
    state.send_player(mimir_audio::PlayerCommand::Stop);
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

/// Snapshot of the live audio player — `None` when the `output` feature
/// is disabled.
#[cfg(feature = "tauri")]
#[derive(Debug, serde::Serialize)]
pub struct PlayerSnapshotOut {
    pub state: String,
    pub current: Option<String>,
    pub next_prepared: Option<String>,
}

#[cfg(feature = "tauri")]
#[tauri::command]
pub fn audio_player_snapshot(
    state: tauri::State<'_, AppState>,
) -> Result<Option<PlayerSnapshotOut>, AppError> {
    let snap = state.player_snapshot().map(PlayerSnapshotOut::from);
    Ok(snap)
}

#[cfg(feature = "tauri")]
impl From<mimir_audio::PlayerSnapshot> for PlayerSnapshotOut {
    fn from(s: mimir_audio::PlayerSnapshot) -> Self {
        Self {
            state: format!("{:?}", s.state),
            current: s.current.as_ref().map(|p| p.display().to_string()),
            next_prepared: s.next_prepared.as_ref().map(|p| p.display().to_string()),
        }
    }
}

/// Diagnostic: dump sample paths from the DB for a given prefix. Used to
/// debug path-normalisation bugs (trailing slash, NFC vs NFD, etc.).
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn library_dump_track_paths(
    state: tauri::State<'_, AppState>,
    like_prefix: String,
) -> Result<Vec<(String, i64)>, AppError> {
    let lib = state.library()?;
    let conn = lib.conn()?;
    let mut stmt =
        conn.prepare("SELECT path, id FROM track WHERE path LIKE ?1 ESCAPE '\\' LIMIT 10")?;
    let rows = stmt.query_map(rusqlite::params![format!("{like_prefix}%")], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let out = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(out)
}

/// Front-end log bridge. The webview can't reach the file-backed logger
/// directly, so JS bubbles its `console.*` calls through here. Levels
/// are freeform strings (`DEBUG`/`INFO`/`WARN`/`ERROR`).
#[cfg(feature = "tauri")]
#[tauri::command]
pub fn app_log(level: String, target: String, message: String) {
    mimir_telemetry::log(&level, &target, &message);
}
