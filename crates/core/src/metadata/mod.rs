//! Metadata extraction: probe, tags, filename heuristics, DB upserts.

mod probe;

#[cfg(test)]
mod tests;

pub use probe::{probe_file, Probe};
