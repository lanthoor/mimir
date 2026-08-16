//! Read-side query layer.
//!
//! Pure functions over `&Connection`. No state, no caching — the front-end
//! owns presentation and paging.

mod albums;
mod tracks;

#[cfg(test)]
mod tests;

pub use albums::{list_albums, AlbumRow};
pub use tracks::{list_tracks, TrackRow};
