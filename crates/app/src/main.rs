//! Mimir host binary.
//!
//! Tier 0 ships a Tauri v2 shell with IPC commands for the Svelte frontend.
//! The actual Tauri builder is wired in `lib.rs`; `main.rs` just calls it.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    mimir_app::run();
}
