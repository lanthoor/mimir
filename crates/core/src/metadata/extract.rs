//! Embedded-tag extraction via `lofty`.

use std::path::Path;

use lofty::file::TaggedFileExt;
use lofty::tag::{ItemKey, Tag};
use thiserror::Error;

use super::probe::read_tagged_file;

/// Tags we care about for browsing / searching.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub composer: Option<String>,
    pub lyrics: Option<String>,
}

#[derive(Debug, Error)]
pub enum ExtractError {
    #[error("lofty: {0}")]
    Lofty(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Extract embedded tags from `path`. Falls back to `Tags::default()` when
/// the file has no tag block or can't be fully parsed; only fails on I/O
/// errors or unknown extensions.
pub fn extract_tags(path: &Path) -> Result<Tags, ExtractError> {
    match read_tagged_file(path) {
        Ok(tagged) => Ok(extract_from_tagged(&tagged)),
        Err(super::probe::ProbeError::Lofty(_)) => Ok(Tags::default()),
        Err(super::probe::ProbeError::Io(io)) => Err(ExtractError::Io(io)),
        Err(super::probe::ProbeError::UnknownExtension(_)) => {
            Err(ExtractError::Lofty("unknown extension".into()))
        }
    }
}

fn extract_from_tagged(tagged: &lofty::file::TaggedFile) -> Tags {
    let Some(primary) = TaggedFileExt::primary_tag(tagged) else {
        return Tags::default();
    };

    Tags {
        title: read_str(primary, &ItemKey::TrackTitle),
        artist: read_str(primary, &ItemKey::TrackArtist),
        album: read_str(primary, &ItemKey::AlbumTitle),
        album_artist: read_str(primary, &ItemKey::AlbumArtist),
        track_no: read_u32(primary, &ItemKey::TrackNumber),
        disc_no: read_u32(primary, &ItemKey::DiscNumber),
        year: read_u32(primary, &ItemKey::Year),
        genre: read_str(primary, &ItemKey::Genre),
        composer: read_str(primary, &ItemKey::Composer),
        lyrics: read_str(primary, &ItemKey::Lyrics),
    }
}

fn read_str(tag: &Tag, key: &ItemKey) -> Option<String> {
    for item in tag.items() {
        if item.key() == key {
            return item.value().text().map(str::to_string);
        }
    }
    None
}

fn read_u32(tag: &Tag, key: &ItemKey) -> Option<u32> {
    for item in tag.items() {
        if item.key() == key {
            return item.value().text().and_then(|s| s.parse::<u32>().ok());
        }
    }
    None
}
