//! `r2d2` connection pool newtype.

use std::path::Path;

use r2d2_sqlite::SqliteConnectionManager;

/// Pool of `SQLite` connections.
#[derive(Debug)]
pub(crate) struct Pool(r2d2::Pool<SqliteConnectionManager>);

impl Pool {
    /// Create an in-memory pool. Used by tests.
    #[cfg(test)]
    pub(crate) fn in_memory() -> Result<Self, r2d2::Error> {
        let manager = SqliteConnectionManager::memory();
        let inner = r2d2::Pool::new(manager)?;
        Ok(Self(inner))
    }

    /// Open (or create) a file-backed pool.
    pub(crate) fn open(path: &Path) -> Result<Self, r2d2::Error> {
        let manager = SqliteConnectionManager::file(path);
        let inner = r2d2::Pool::new(manager)?;
        Ok(Self(inner))
    }

    pub(crate) fn get(
        &self,
    ) -> Result<r2d2::PooledConnection<SqliteConnectionManager>, r2d2::Error> {
        self.0.get()
    }
}
