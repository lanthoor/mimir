//! Domain events emitted by the file watcher.

use std::path::PathBuf;

/// What happened to a file on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventKind {
    /// The file was just created.
    Created,
    /// The file was modified (mtime/content changed).
    Modified,
    /// The file was removed.
    Removed,
    /// The file was renamed from `from` to `to`.
    Renamed { from: PathBuf, to: PathBuf },
}

/// A single ingest-relevant filesystem event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestEvent {
    pub path: PathBuf,
    pub kind: EventKind,
}
