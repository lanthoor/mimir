//! SQLite-backed library store.

mod cover_art;
mod error;
mod library;
mod migrations;
mod pool;

#[cfg(test)]
mod tests;

pub use cover_art::{album_cover, attach_album_cover, detach_album_cover, CoverRow};
pub use error::DbError;
pub use library::Library;
