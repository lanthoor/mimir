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
        artist: read_artist(primary, &ItemKey::TrackArtist),
        album: read_str(primary, &ItemKey::AlbumTitle),
        album_artist: read_artist(primary, &ItemKey::AlbumArtist),
        track_no: read_track_no(primary, &ItemKey::TrackNumber),
        disc_no: read_track_no(primary, &ItemKey::DiscNumber),
        year: read_year(primary),
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
    tag.get_string(key).map(str::to_string)
}

/// `read_str` with a trim — some taggers (esp. for `ARTIST`) pad with
/// zero-width spaces or trailing whitespace that confuses downstream joins
/// and equality checks.
fn read_artist(tag: &Tag, key: &ItemKey) -> Option<String> {
    read_str(tag, key)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Read a track or disc number. Tag values can be `"3"`, `"3/12"` (track
/// 3 of 12), or even `"03"` — we take the part before `/` and parse.
fn read_track_no(tag: &Tag, key: &ItemKey) -> Option<u32> {
    let s = read_str(tag, key)?;
    let first = s.split('/').next().unwrap_or(s.as_str());
    first.trim().parse::<u32>().ok()
}

/// Read a year. Lofty splits this into two `ItemKey`s: `Year` (4-digit
/// string, `ID3v2` TYER-style) and `RecordingDate` (full ISO timestamp or
/// `YYYY`, e.g. `"2024-03-15"`). Many formats (Vorbis, MP4, AIFF) only
/// expose `RecordingDate`. Prefer `RecordingDate` when present — it carries
/// the full date and is more specific. Fall back to `Year` for `ID3v2.3`
/// files that only have TYER.
fn read_year(tag: &Tag) -> Option<u32> {
    for key in [&ItemKey::RecordingDate, &ItemKey::Year] {
        if let Some(text) = read_str(tag, key) {
            if let Some(year) = parse_year_string(&text) {
                return Some(year);
            }
        }
    }
    None
}

fn parse_year_string(text: &str) -> Option<u32> {
    // Take the first 4 consecutive digits from the leading numeric chunk.
    // Handles: "2024", "2024-03-15", "2024/03/15", "20240315".
    let mut digits = String::with_capacity(4);
    for c in text.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            if digits.len() == 4 {
                break;
            }
        } else if !digits.is_empty() {
            // First non-digit after digits terminates the year component.
            break;
        }
    }
    if digits.len() != 4 {
        return None;
    }
    digits.parse::<u32>().ok()
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

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::tag::TagType;

    #[test]
    fn parse_year_handles_iso_date() {
        assert_eq!(parse_year_string("2024-03-15"), Some(2024));
        assert_eq!(parse_year_string("2024/03/15"), Some(2024));
        assert_eq!(parse_year_string("2024"), Some(2024));
        assert_eq!(parse_year_string("20240315"), Some(2024));
        assert_eq!(parse_year_string("1999-12-31T23:59:59"), Some(1999));
    }

    #[test]
    fn parse_year_rejects_non_years() {
        assert_eq!(parse_year_string(""), None);
        assert_eq!(parse_year_string("abc"), None);
        assert_eq!(parse_year_string("24"), None); // 2 digits is not a year
    }

    #[test]
    fn read_year_falls_back_to_recording_date() {
        // RecordingDate is the common Vorbis/MP4 field; year must be read from it.
        let mut tag = Tag::new(TagType::VorbisComments);
        tag.insert_text(ItemKey::RecordingDate, "2023-06-12".into());
        assert_eq!(read_year(&tag), Some(2023));

        // Vorbis files with the older `YEAR=` (rare) should also work.
        let mut tag = Tag::new(TagType::VorbisComments);
        tag.insert_text(ItemKey::Year, "1995".into());
        assert_eq!(read_year(&tag), Some(1995));

        // Both set — RecordingDate wins because it carries the full date.
        let mut tag = Tag::new(TagType::VorbisComments);
        tag.insert_text(ItemKey::Year, "2000".into());
        tag.insert_text(ItemKey::RecordingDate, "2024".into());
        assert_eq!(read_year(&tag), Some(2024));
    }

    #[test]
    fn read_track_no_handles_total_format() {
        let mut tag = Tag::new(TagType::Id3v2);
        tag.insert_text(ItemKey::TrackNumber, "3/12".into());
        assert_eq!(read_track_no(&tag, &ItemKey::TrackNumber), Some(3));

        let mut tag = Tag::new(TagType::Id3v2);
        tag.insert_text(ItemKey::TrackNumber, " 7 ".into());
        assert_eq!(read_track_no(&tag, &ItemKey::TrackNumber), Some(7));

        let mut tag = Tag::new(TagType::Id3v2);
        tag.insert_text(ItemKey::TrackNumber, "nope".into());
        assert_eq!(read_track_no(&tag, &ItemKey::TrackNumber), None);
    }

    #[test]
    fn read_artist_trims_and_skips_empty() {
        let mut tag = Tag::new(TagType::Id3v2);
        tag.insert_text(ItemKey::TrackArtist, "  Björk  ".into());
        assert_eq!(
            read_artist(&tag, &ItemKey::TrackArtist),
            Some("Björk".into())
        );

        let mut tag = Tag::new(TagType::Id3v2);
        tag.insert_text(ItemKey::TrackArtist, "   ".into());
        assert_eq!(read_artist(&tag, &ItemKey::TrackArtist), None);
    }
}
