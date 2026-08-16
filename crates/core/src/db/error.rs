//! Database error type.

use thiserror::Error;

/// Errors produced by the SQLite-backed library store.
#[derive(Debug, Error)]
pub enum DbError {
    /// `rusqlite` returned an error (constraint, I/O, schema, etc.).
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// `r2d2` could not hand out a connection (pool exhausted, broken, etc.).
    #[error("connection pool: {0}")]
    Pool(#[from] r2d2::Error),
}
