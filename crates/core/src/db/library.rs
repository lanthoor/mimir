//! Top-level library handle.

use std::path::Path;

use crate::db::error::DbError;
use crate::db::migrations;
use crate::db::pool::Pool;

/// An open Mimir library, backed by a `SQLite` connection pool.
#[derive(Debug, Clone)]
pub struct Library {
    pool: Pool,
}

impl Library {
    /// Open (or create) the library at `path`, applying any pending migrations.
    ///
    /// The file is created if missing. WAL journaling is enabled.
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let pool = Pool::open(path)?;
        let lib = Self { pool };
        lib.init()?;
        Ok(lib)
    }

    /// Open an in-memory library (tests only).
    #[cfg(test)]
    pub(crate) fn in_memory() -> Result<Self, DbError> {
        let pool = Pool::in_memory()?;
        let lib = Self { pool };
        lib.init()?;
        Ok(lib)
    }

    /// Borrow a connection from the pool.
    pub fn conn(
        &self,
    ) -> Result<r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>, DbError> {
        Ok(self.pool.get()?)
    }

    fn init(&self) -> Result<(), DbError> {
        let conn = self.conn()?;
        Self::configure(&conn)?;
        let _applied = migrations::ensure_and_apply(&conn)?;
        Ok(())
    }

    fn configure(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
        // WAL: better concurrency between scanner / watcher / query.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // FKs: respect declared relationships.
        conn.pragma_update(None, "foreign_keys", "ON")?;
        // Wait up to 5s for a contended lock before failing.
        conn.pragma_update(None, "busy_timeout", 5_000)?;
        Ok(())
    }
}
