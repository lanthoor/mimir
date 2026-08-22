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

/// Entry point invoked from `main.rs`. Wraps the Tauri builder so the
/// library + tests can be built without the GTK/webkit2gtk system deps.
#[cfg(feature = "tauri")]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::new())
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
            command::library_folder_tree,
            command::library_search,
            command::library_list_albums,
            command::library_list_genres,
            command::library_list_years,
            command::library_list_tracks,
            command::library_query_tracks,
            command::library_album_cover,
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
