//! Metadata extraction.
//!
//! Reads tags from audio files via `lofty`, falls back to filename heuristics
//! when tags are missing, and upserts artist / album / track rows.

#[cfg(test)]
mod tests;
