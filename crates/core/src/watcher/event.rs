//! Domain events emitted by the file watcher.

use std::path::Path;
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

/// Lower-case file extensions we recognise as audio.
///
/// Alac is identified by extension only here; the codec string in the `track`
/// row is decided by P4 metadata extraction.
const AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "aiff", "aif", "alac",
];

/// True if `path` has an extension we treat as audio. Case-insensitive.
pub fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| {
            let lower = ext.to_ascii_lowercase();
            AUDIO_EXTS.iter().any(|known| *known == lower)
        })
}
