//! SQLite-backed library store.

mod error;
mod library;
mod pool;

#[cfg(test)]
mod tests;

pub use error::DbError;
pub use library::Library;
pub(crate) use pool::Pool;
