//! Mimir host binary.
//!
//! Phase 0 (CI Bootstrap) prints a version banner so the CI pipeline has a
//! runnable artifact to produce. The real Tauri shell lands in Tier 0.

fn main() {
    println!("mimir {}", env!("CARGO_PKG_VERSION"));
}
