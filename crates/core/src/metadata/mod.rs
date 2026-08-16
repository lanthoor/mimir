//! Metadata extraction: probe, tags, filename heuristics, DB upserts.

mod extract;
mod heuristic;
mod probe;
mod upsert;

#[cfg(test)]
mod tests;

pub use extract::{extract_tags, Tags};
pub use heuristic::{parse_filename, HeuristicTags};
pub use probe::{probe_file, Probe};
pub use upsert::{upsert_album, upsert_artist};
