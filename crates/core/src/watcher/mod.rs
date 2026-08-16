//! Cross-platform file-system watcher.
//!
//! Wraps `notify` + `notify-debouncer-full` and produces a stream of
//! `IngestEvent`s that other parts of the core pipeline (scanner, metadata
//! extractor) can consume off an `mpsc::Receiver`.

mod event;

#[cfg(test)]
mod tests;

pub use event::{EventKind, IngestEvent};
