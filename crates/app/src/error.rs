//! Error type returned from IPC commands. Serializable so the front-end
//! gets a structured failure, not a panic.

use mimir_core::r2d2;
use mimir_core::rusqlite;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum AppError {
    #[error("io: {0}")]
    Io(String),
    #[error("sqlite: {0}")]
    Sqlite(String),
    #[error("decode: {0}")]
    Decode(String),
    #[error("path not found: {0}")]
    PathNotFound(String),
    #[error("internal: {0}")]
    Internal(String),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Sqlite(e.to_string())
    }
}

impl From<r2d2::Error> for AppError {
    fn from(e: r2d2::Error) -> Self {
        Self::Internal(format!("connection pool: {e}"))
    }
}

impl From<mimir_core::db::DbError> for AppError {
    fn from(e: mimir_core::db::DbError) -> Self {
        match e {
            mimir_core::db::DbError::Sqlite(s) => Self::Sqlite(s.to_string()),
            mimir_core::db::DbError::Pool(s) => Self::Internal(format!("pool: {s}")),
        }
    }
}
