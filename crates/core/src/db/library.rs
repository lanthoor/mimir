//! Top-level library handle.

use std::path::Path;

use crate::db::error::DbError;
use crate::db::pool::Pool;

/// An open Mimir library, backed by a SQLite connection pool.
#[derive(Debug)]
pub struct Library {
    pool: Pool,
}

impl Library {
    /// Open (or create) the library at `path`.
    ///
    /// The file is created if missing. Migrations are applied on first open.
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let pool = Pool::open(path)?;
        Ok(Self { pool })
    }

    /// Open an in-memory library (tests only).
    #[cfg(test)]
    pub(crate) fn in_memory() -> Result<Self, DbError> {
        let pool = Pool::in_memory()?;
        Ok(Self { pool })
    }

    /// Borrow a connection from the pool.
    pub fn conn(&self) -> Result<r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>, DbError> {
        Ok(self.pool.get()?)
    }
}
