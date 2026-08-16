//! Mimir host binary.
//!
//! Phase 0 (CI Bootstrap) prints a version banner so the CI pipeline has a
//! runnable artifact to produce. The real Tauri shell lands in Tier 0.

use mimir_audio::hello as audio_hello;
use mimir_core::hello as core_hello;

fn main() {
    println!(
        "mimir {} (core: {}, audio: {})",
        env!("CARGO_PKG_VERSION"),
        core_hello(),
        audio_hello(),
    );
}
