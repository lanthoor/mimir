//! Read-side query layer.
//!
//! Pure functions over `&Connection`. No state, no caching — the front-end
//! owns presentation and paging.

mod albums;
mod artists;
mod filter;
mod folders;
mod search;
mod tracks;

#[cfg(test)]
mod tests;

pub use albums::{count_albums, list_albums, AlbumRow};
pub use artists::{count_artists, list_artists, ArtistRow};
pub use filter::{count_tracks_filtered, list_tracks_filtered, TrackFilter};
pub use folders::{
    count_folder_files, count_listed_folders, list_folder_files, list_folders, list_listed_folders,
    FolderFile, FolderNode, FolderView, ListFolderRow,
};
pub use search::{search_tracks, search_tracks_page, TrackSearchPage};
pub use tracks::{
    count_genres, count_tracks, count_years, list_genres, list_tracks, list_years, GenreRow,
    TrackRow, YearRow,
};
