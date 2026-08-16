//! SQLite-backed library store.

mod error;
mod library;
mod migrations;
mod pool;

#[cfg(test)]
mod tests;

pub use error::DbError;
pub use library::Library;
