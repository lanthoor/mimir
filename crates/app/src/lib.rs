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
        .manage(state::AppState::new())
        .invoke_handler(tauri::generate_handler![
            command::library_open,
            command::library_status,
            command::library_add_folder,
            command::library_search,
            command::library_list_albums,
            command::library_list_genres,
            command::library_list_years,
            command::library_query_tracks,
            command::library_album_cover,
            command::audio_play,
            command::audio_pause,
            command::audio_resume,
            command::audio_stop,
            command::audio_next,
            command::audio_previous,
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
