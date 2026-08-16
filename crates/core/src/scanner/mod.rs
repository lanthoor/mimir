//! Directory scanner.
//!
//! Walks filesystem trees under watched roots, computes dedupe keys, and
//! upserts folder rows. Emits `ScanJob`s to a channel for downstream
//! metadata extraction.

mod dedupe;
mod hash;
mod upsert;
mod walk;

#[cfg(test)]
mod tests;

pub use dedupe::{dedupe_key, ScanJob};
pub use hash::hash_file;
pub use upsert::upsert_folder;
pub use walk::walk_audio_files;
