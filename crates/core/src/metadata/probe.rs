//! Codec + audio-property probe via `lofty`.

use std::path::Path;

use lofty::file::{AudioFile, FileType, TaggedFile, TaggedFileExt};
use lofty::picture::Picture;
use lofty::probe::Probe as LoftyProbe;
use mimir_telemetry as telemetry;
use thiserror::Error;

use super::cover::{select_cover, CoverArt};

/// Coarse audio properties extracted from the container header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    /// Lower-case codec string: `mp3`, `flac`, `vorbis`, `opus`, `wav`, …
    pub codec: String,
    /// Track duration in milliseconds, if known.
    pub duration_ms: Option<i32>,
    /// Sample rate in Hz, if known.
    pub sample_rate: Option<i32>,
    /// Channel count, if known.
    pub channels: Option<u8>,
    /// Bitrate in bits per second, if known.
    pub bitrate: Option<i32>,
}

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("lofty: {0}")]
    Lofty(String),
    #[error("unknown file extension: {0:?}")]
    UnknownExtension(String),
}

/// Probe `path` for codec + audio properties.
///
/// Returns `Err` when the file format cannot be recognised either from the
/// extension or from the on-disk magic bytes.
pub fn probe_file(path: &Path) -> Result<Probe, ProbeError> {
    telemetry::log(
        "DEBUG",
        "metadata",
        &format!("probe_file start path={}", path.display()),
    );
    let codec_hint = codec_from_extension(path).map(str::to_string);

    let tagged = match read_tagged_file(path) {
        Ok(t) => {
            telemetry::log(
                "DEBUG",
                "metadata",
                &format!(
                    "probe_file tagged ok path={} hint={}",
                    path.display(),
                    codec_hint.as_deref().unwrap_or("?")
                ),
            );
            t
        }
        Err(e) => {
            telemetry::log(
                "WARN",
                "metadata",
                &format!(
                    "probe_file read_tagged_file failed path={} err={e}",
                    path.display()
                ),
            );
            if let Some(c) = codec_hint {
                telemetry::log(
                    "DEBUG",
                    "metadata",
                    &format!("probe_file fallback to extension codec={c}"),
                );
                return Ok(Probe {
                    codec: c,
                    duration_ms: None,
                    sample_rate: None,
                    channels: None,
                    bitrate: None,
                });
            }
            return Err(e);
        }
    };

    let p = Probe::from_tagged(&tagged);
    telemetry::log(
        "DEBUG",
        "metadata",
        &format!(
            "probe_file done path={} codec={} dur={:?} sr={:?} ch={:?} br={:?}",
            path.display(),
            p.codec,
            p.duration_ms,
            p.sample_rate,
            p.channels,
            p.bitrate
        ),
    );
    Ok(p)
}

pub(crate) fn read_tagged_file(path: &Path) -> Result<TaggedFile, ProbeError> {
    let mut probe = LoftyProbe::open(path).map_err(|e| ProbeError::Lofty(e.to_string()))?;
    if probe.file_type().is_none() {
        probe = probe.guess_file_type().map_err(ProbeError::Io)?;
    }
    probe.read().map_err(|e| ProbeError::Lofty(e.to_string()))
}

/// Read the embedded cover art from `path`, if any.
///
/// Returns `Ok(None)` when the file has no pictures or cannot be parsed by
/// `lofty`; returns `Err` only on I/O failure.
pub fn extract_cover(path: &Path) -> Result<Option<CoverArt>, ProbeError> {
    telemetry::log(
        "DEBUG",
        "metadata",
        &format!("extract_cover path={}", path.display()),
    );
    let tagged = match read_tagged_file(path) {
        Ok(t) => t,
        Err(e @ ProbeError::Io(_)) => return Err(e),
        Err(e) => {
            telemetry::log(
                "DEBUG",
                "metadata",
                &format!("extract_cover no tag block err={e}"),
            );
            return Ok(None);
        }
    };
    let Some(primary) = TaggedFileExt::primary_tag(&tagged) else {
        return Ok(None);
    };
    let pictures: Vec<&Picture> = primary.pictures().iter().collect();
    let picked = select_cover(&pictures);
    if picked.is_some() {
        telemetry::log(
            "DEBUG",
            "metadata",
            &format!("extract_cover found count={} picked=yes", pictures.len()),
        );
    }
    Ok(picked)
}

impl Probe {
    fn from_tagged(tagged: &TaggedFile) -> Self {
        let codec = codec_name(TaggedFileExt::file_type(tagged)).to_string();
        let props = tagged.properties();
        let duration_ms = i64::try_from(props.duration().as_millis())
            .ok()
            .and_then(|v| i32::try_from(v).ok());
        let sample_rate = props.sample_rate().and_then(|s| i32::try_from(s).ok());
        let channels = props.channels();
        let bitrate = props.audio_bitrate().and_then(|b| i32::try_from(b).ok());

        Probe {
            codec,
            duration_ms,
            sample_rate,
            channels,
            bitrate,
        }
    }
}

pub(crate) fn codec_name(ft: FileType) -> &'static str {
    match ft {
        FileType::Mpeg => "mp3",
        FileType::Flac => "flac",
        FileType::Vorbis => "vorbis",
        FileType::Opus => "opus",
        FileType::Wav => "wav",
        FileType::Aiff => "aiff",
        FileType::Mp4 => "mp4",
        FileType::Aac => "aac",
        _ => "unknown",
    }
}

pub(crate) fn codec_from_extension(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "mp3" => Some("mp3"),
        "flac" => Some("flac"),
        "ogg" => Some("vorbis"),
        "opus" => Some("opus"),
        "wav" => Some("wav"),
        "aiff" | "aif" => Some("aiff"),
        "m4a" | "mp4" => Some("mp4"),
        "aac" => Some("aac"),
        "alac" => Some("alac"),
        _ => None,
    }
}
