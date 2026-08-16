//! Codec + audio-property probe via `lofty`.

use std::path::Path;

use lofty::file::{AudioFile, FileType, TaggedFile, TaggedFileExt};
use lofty::probe::Probe as LoftyProbe;
use thiserror::Error;

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
    let codec_hint = codec_from_extension(path).map(str::to_string);

    let tagged = match read_tagged_file(path) {
        Ok(t) => t,
        Err(e) => {
            if let Some(c) = codec_hint {
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

    Ok(Probe::from_tagged(&tagged))
}

pub(crate) fn read_tagged_file(path: &Path) -> Result<TaggedFile, ProbeError> {
    let mut probe = LoftyProbe::open(path).map_err(|e| ProbeError::Lofty(e.to_string()))?;
    if probe.file_type().is_none() {
        probe = probe.guess_file_type().map_err(ProbeError::Io)?;
    }
    probe.read().map_err(|e| ProbeError::Lofty(e.to_string()))
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
