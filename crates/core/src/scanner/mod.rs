//! Directory scanner.
//!
//! Walks filesystem trees under watched roots, computes dedupe keys, and
//! upserts folder rows. Emits `ScanJob`s to a channel for downstream
//! metadata extraction.

mod walk;

#[cfg(test)]
mod tests;

pub use walk::walk_audio_files;
