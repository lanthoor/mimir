//! Per-file hashing + metadata for dedupe.

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use thiserror::Error;

/// 32-byte blake3 content hash of an audio file plus the filesystem
/// metadata used for change detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHash {
    /// blake3 hash of the file bytes.
    pub path_hash: [u8; 32],
    /// Modification time in nanoseconds since the Unix epoch.
    pub mtime_ns: i64,
    /// File size in bytes.
    pub size_bytes: i64,
}

#[derive(Debug, Error)]
pub enum HashError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("mtime predates unix epoch")]
    MtimeBeforeEpoch,
}

/// Compute the blake3 hash of `path`'s contents and capture its mtime + size.
///
/// Used both for dedupe (the `(path_hash, mtime_ns, size_bytes)` triple is
/// cheap to compare and survives renames) and as a stable identifier of the
/// file's bytes.
pub fn hash_file(path: &Path) -> Result<FileHash, HashError> {
    let bytes = fs::read(path)?;
    let path_hash: [u8; 32] = blake3::hash(&bytes).into();

    let meta = fs::metadata(path)?;
    let size_bytes = meta.len() as i64;
    let mtime = meta
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_err(|_| HashError::MtimeBeforeEpoch)?;
    let mtime_ns = i64::try_from(mtime.as_nanos()).map_err(|_| HashError::MtimeBeforeEpoch)?;

    Ok(FileHash {
        path_hash,
        mtime_ns,
        size_bytes,
    })
}
