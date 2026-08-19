//! Embedded-tag extraction via `lofty`.

use lofty::file::TaggedFileExt;
use lofty::tag::{ItemKey, Tag};
use std::path::Path;
use thiserror::Error;

use super::probe::read_tagged_file;

/// Tags we care about for browsing / searching.
#[derive(Debug, Clone, Default, PartialEq)]
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
    /// `ReplayGain` track gain in dB. `None` when absent.
    pub replaygain_track_db: Option<f64>,
    /// `ReplayGain` album gain in dB. `None` when absent.
    pub replaygain_album_db: Option<f64>,
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
    mimir_telemetry::log(
        "DEBUG",
        "metadata",
        &format!("extract_tags start path={}", path.display()),
    );
    let tags = match read_tagged_file(path) {
        Ok(tagged) => {
            let t = extract_from_tagged(&tagged);
            mimir_telemetry::log(
                "DEBUG",
                "metadata",
                &format!(
                    "extract_tags ok path={} title={:?} artist={:?} album={:?} genre={:?} year={:?} rg_track={:?} rg_album={:?}",
                    path.display(),
                    t.title,
                    t.artist,
                    t.album,
                    t.genre,
                    t.year,
                    t.replaygain_track_db,
                    t.replaygain_album_db
                ),
            );
            t
        }
        Err(super::probe::ProbeError::Lofty(e)) => {
            mimir_telemetry::log(
                "DEBUG",
                "metadata",
                &format!(
                    "extract_tags lofty-err fallback default path={} err={e}",
                    path.display()
                ),
            );
            Tags::default()
        }
        Err(super::probe::ProbeError::Io(io)) => {
            mimir_telemetry::log(
                "ERROR",
                "metadata",
                &format!("extract_tags io err path={} err={io}", path.display()),
            );
            return Err(ExtractError::Io(io));
        }
        Err(super::probe::ProbeError::UnknownExtension(_)) => {
            return Err(ExtractError::Lofty("unknown extension".into()));
        }
    };
    Ok(tags)
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
        replaygain_track_db: parse_replaygain(
            primary,
            "REPLAYGAIN_TRACK_GAIN",
            "replaygain_track_gain",
        ),
        replaygain_album_db: parse_replaygain(
            primary,
            "REPLAYGAIN_ALBUM_GAIN",
            "replaygain_album_gain",
        ),
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

/// Parse a `ReplayGain` dB value from any tag item whose key matches
/// either the Vorbis-style upper-case form (e.g. `REPLAYGAIN_TRACK_GAIN`)
/// or the `ID3v2` TXXX description lower-case form (e.g.
/// `replaygain_track_gain`). Returns `None` if missing or invalid.
pub fn parse_replaygain(tag: &Tag, vorbis_key: &str, txxx_desc: &str) -> Option<f64> {
    for item in tag.items() {
        if let ItemKey::Unknown(name) = item.key() {
            if name == vorbis_key || name.eq_ignore_ascii_case(txxx_desc) {
                if let Some(text) = item.value().text() {
                    if let Some(db) = parse_db_string(text) {
                        return Some(db);
                    }
                }
            }
        }
    }
    None
}

/// Parse e.g. `"-6.84 dB"` → `-6.84`.
fn parse_db_string(text: &str) -> Option<f64> {
    let trimmed = text.trim().trim_end_matches(" dB").trim_end_matches("dB");
    let cleaned: String = trimmed
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',')
        .collect();
    cleaned.parse().ok()
}
