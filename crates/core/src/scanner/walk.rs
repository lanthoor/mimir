//! Recursive walk that yields only audio files.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::watcher::is_audio_path;

/// Walk `root` recursively and yield paths whose extension is in the
/// audio whitelist. Symlinks are not followed.
pub fn walk_audio_files(root: &Path) -> impl Iterator<Item = PathBuf> {
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| is_audio_path(p))
}
