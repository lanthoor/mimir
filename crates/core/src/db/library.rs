//! Top-level library handle.

use std::path::Path;

use mimir_telemetry as telemetry;

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
        telemetry::log(
            "INFO",
            "db",
            &format!("Library::open start path={}", path.display()),
        );
        let pool = Pool::open(path)?;
        let lib = Self { pool };
        if let Err(e) = lib.init() {
            telemetry::log(
                "ERROR",
                "db",
                &format!("Library::init failed path={} err={}", path.display(), e),
            );
            return Err(e);
        }
        telemetry::log(
            "INFO",
            "db",
            &format!("Library::open ok path={}", path.display()),
        );
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
        match self.pool.get() {
            Ok(c) => Ok(c),
            Err(e) => {
                telemetry::log("ERROR", "db", &format!("connection acquire failed: {e}"));
                Err(DbError::from(e))
            }
        }
    }

    fn init(&self) -> Result<(), DbError> {
        telemetry::log("DEBUG", "db", "init: configure pragmas");
        let conn = self.conn()?;
        Self::configure(&conn)?;
        telemetry::log("DEBUG", "db", "init: ensure_and_apply");
        let applied = migrations::ensure_and_apply(&conn)?;
        telemetry::log(
            "INFO",
            "db",
            &format!("init: schema_version now {applied}"),
        );
        Ok(())
    }

    fn configure(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
        // WAL: better concurrency between scanner / watcher / query.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        telemetry::log("DEBUG", "db", "pragma journal_mode=WAL");
        // FKs: respect declared relationships.
        conn.pragma_update(None, "foreign_keys", "ON")?;
        // Wait up to 5s for a contended lock before failing.
        conn.pragma_update(None, "busy_timeout", 5_000)?;
        Ok(())
    }
}
