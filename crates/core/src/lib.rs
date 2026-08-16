//! Mimir core crate.

pub mod db;
pub mod metadata;
pub mod query;
pub mod scanner;
pub mod watcher;

pub use r2d2;
/// Re-exports of error types from underlying crates so the host binary
/// can `impl From<…>` without taking a direct dependency on them.
pub use rusqlite;
