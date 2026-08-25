//! Mimir app crate.
//!
//! Houses the Tauri shell + the IPC command set. The audio engine and core
//! library live in their own crates; this crate is the host binary's seam.

mod command;
mod error;
mod state;

#[cfg(test)]
mod tests;

pub use error::AppError;
pub use state::AppState;

/// Pull the album id out of a `mimircover://localhost/cover/{id}` path.
/// Kept as a free function (not feature-gated) so it's testable without
/// the GTK/webkit2gtk deps and out of the request-handling closure.
/// `.path()` already drops any query string, so `/cover/5?x=1` → `5`.
pub fn cover_id_from_path(path: &str) -> Option<i64> {
    path.strip_prefix("/cover/")
        .and_then(|id| id.parse::<i64>().ok())
}

/// Entry point invoked from `main.rs`. Wraps the Tauri builder so the
/// library + tests can be built without the GTK/webkit2gtk system deps.
#[cfg(feature = "tauri")]
pub fn run() {
    use tauri::http::{header::CONTENT_TYPE, Request, Response, StatusCode};
    use tauri::Manager;

    /// Serve `mimircover://localhost/cover/{album_id}` so the webview can
    /// load cover art as a plain resource — no JSON round-trip, no base64,
    /// no main-thread serialization of megabytes (which is what froze the
    /// Albums view).
    fn cover_response(request: Request<Vec<u8>>, app: &tauri::AppHandle) -> Response<Vec<u8>> {
        let id = cover_id_from_path(request.uri().path());

        let row = id.and_then(|id| {
            let state: tauri::State<AppState> = app.state();
            let lib = state.library().ok()?;
            let conn = lib.conn().ok()?;
            mimir_core::db::album_cover(&conn, id).ok()?
        });

        match row {
            Some(row) => Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, row.mime_type.as_str())
                .header("Cache-Control", "public, max-age=3600")
                .body(row.data)
                .expect("cover response builder"),
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new())
                .expect("404 response builder"),
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::new())
        .register_uri_scheme_protocol("mimircover", |ctx, request| {
            cover_response(request, ctx.app_handle())
        })
        .invoke_handler(tauri::generate_handler![
            command::library_open,
            command::library_status,
            command::library_add_folder,
            command::library_add_folders,
            command::library_remove_folder,
            command::library_rename_folder,
            command::library_rename_subdir,
            command::library_reveal_in_file_manager,
            command::library_list_folders,
            command::library_list_listed_folders,
            command::library_count_listed_folders,
            command::library_list_folder_files,
            command::library_count_folder_files,
            command::library_folder_tree,
            command::library_search,
            command::library_search_tracks_page,
            command::library_list_albums,
            command::library_count_albums,
            command::library_list_artists,
            command::library_count_artists,
            command::library_list_genres,
            command::library_count_genres,
            command::library_list_years,
            command::library_count_years,
            command::library_list_tracks,
            command::library_count_tracks,
            command::library_query_tracks,
            command::library_count_tracks_filtered,
            command::library_get_editable_track,
            command::library_update_track,
            command::library_clear_track_field,
            command::library_track_lyrics,
            command::audio_play,
            command::audio_pause,
            command::audio_resume,
            command::audio_stop,
            command::audio_next,
            command::audio_previous,
            command::audio_player_snapshot,
            command::audio_queue_enqueue_many,
            command::audio_play_and_enqueue_many,
            command::audio_queue_remove_at,
            command::audio_queue_move,
            command::audio_queue_play_at,
            command::audio_queue_clear,
            command::audio_queue_get,
            command::app_log,
            command::library_dump_track_paths,
        ])
        .run(tauri::generate_context!())
        .expect("mimir app failed to start");
}

/// Stubs out `run()` when Tauri is not built (no feature) so the binary
/// can still be produced — useful for `cargo check` in CI.
#[cfg(not(feature = "tauri"))]
pub fn run() {
    eprintln!("mimir-app built without the `tauri` feature; enable it to launch the GUI.");
}
