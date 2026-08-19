//! Read-side query layer.
//!
//! Pure functions over `&Connection`. No state, no caching — the front-end
//! owns presentation and paging.

mod albums;
mod artists;
mod filter;
mod search;
mod tracks;

#[cfg(test)]
mod tests;

pub use albums::{list_albums, AlbumRow};
pub use artists::{list_artists, ArtistRow};
pub use filter::{list_tracks_filtered, TrackFilter};
pub use search::search_tracks;
pub use tracks::{list_genres, list_tracks, list_years, GenreRow, TrackRow, YearRow};
