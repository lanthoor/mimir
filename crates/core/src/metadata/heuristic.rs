//! Filename-based fallback when no embedded tags are present.
//!
//! Recognises the common `<Artist>/<Album>/<TrackNo> - <Title>.<ext>` layout
//! used by music libraries. Falls back to `None` for anything ambiguous —
//! better to have no tags than wrong ones.

use std::path::Path;

/// Tags recovered from a filename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeuristicTags {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_no: Option<u32>,
    pub title: Option<String>,
}

/// Try to parse the conventional `Artist/Album/TrackNo - Title.ext` layout.
///
/// `path` should be a leaf filename *and* its parent directories — the
/// layout relies on at least two parent components.
pub fn parse_filename(path: &Path) -> Option<HeuristicTags> {
    let components: Vec<String> = path
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => s.to_str().map(str::to_string),
            _ => None,
        })
        .collect();

    if components.is_empty() {
        return None;
    }

    // Drop the file extension from the last component.
    let mut last = components.last()?.clone();
    if let Some(dot) = last.rfind('.') {
        last.truncate(dot);
    }
    let mut parts = components;
    let n = parts.len();
    parts[n - 1] = last;

    // Need at least 2 ancestors: grandparent = artist, parent = album.
    if n < 3 {
        return None;
    }

    let artist = parts[n - 3].clone();
    let album = parts[n - 2].clone();
    let leaf = parts[n - 1].clone();

    let (track_no, title) = parse_leaf(&leaf);

    Some(HeuristicTags {
        artist: Some(artist),
        album: Some(album),
        track_no,
        title,
    })
}

/// Parse the leaf filename of the form `NN - Title` into its pieces.
///
/// Returns `(None, Some(leaf))` when no track number is present.
fn parse_leaf(leaf: &str) -> (Option<u32>, Option<String>) {
    // Split on the first ` - `.
    if let Some(idx) = leaf.find(" - ") {
        let (left, right) = leaf.split_at(idx);
        let right = &right[3..]; // skip " - "
        if let Ok(n) = left.trim().parse::<u32>() {
            return (Some(n), Some(right.trim().to_string()));
        }
    }
    (None, Some(leaf.trim().to_string()))
}
