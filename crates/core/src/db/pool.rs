//! `r2d2` connection pool newtype.

use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;

/// Pool of SQLite connections.
#[derive(Debug)]
pub(crate) struct Pool(r2d2::Pool<SqliteConnectionManager>);

impl Pool {
    /// Create an in-memory pool. Used by tests.
    pub(crate) fn in_memory() -> Result<Self, r2d2::Error> {
        let manager = SqliteConnectionManager::memory();
        let inner = r2d2::Pool::new(manager)?;
        Ok(Self(inner))
    }

    /// Open (or create) a file-backed pool.
    pub(crate) fn open(path: &std::path::Path) -> Result<Self, r2d2::Error> {
        let manager = SqliteConnectionManager::file(path);
        let inner = r2d2::Pool::new(manager)?;
        Ok(Self(inner))
    }

    pub(crate) fn get(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>, r2d2::Error> {
        self.0.get()
    }

    /// Hand out a raw connection (bypassing the pool). Tests use this to
    /// inspect schema state without needing a second pool.
    #[cfg(test)]
    pub(crate) fn open_one_off(path: &std::path::Path) -> Result<Connection, rusqlite::Error> {
        Connection::open(path)
    }
}
