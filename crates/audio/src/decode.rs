//! Decoding audio files to interleaved f32 PCM via `symphonia`.
//!
//! The decoder is intentionally minimal: one pass through the file, no
//! seeking, no resampling. DSP and output wiring happen in P7.

use std::path::Path;

use symphonia::core::audio::{AudioBuffer, AudioBufferRef};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::probe::Hint;
use thiserror::Error;

/// Interleaved f32 PCM samples.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioBufferOut {
    /// Interleaved f32 samples in `[-1.0, 1.0]`.
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported format")]
    UnsupportedFormat,
    #[error("no audio tracks")]
    NoTracks,
    #[error("decode: {0}")]
    Decode(String),
    #[error("too many channels: {0}")]
    TooManyChannels(usize),
}

const fn unsupported(err: &SymError) -> bool {
    matches!(err, SymError::Unsupported(_))
}

/// Decode the audio file at `path` to interleaved f32 PCM.
///
/// Decodes the *first* audio track in the container to completion. For Tier
/// 0's single-track-per-file assumption this is sufficient.
pub fn decode_file(path: &Path) -> Result<AudioBufferOut, DecodeError> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &symphonia::core::meta::MetadataOptions::default(),
        )
        .map_err(|e| {
            if unsupported(&e) {
                DecodeError::UnsupportedFormat
            } else {
                DecodeError::Decode(e.to_string())
            }
        })?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(DecodeError::NoTracks)?;

    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44_100);
    let channel_count = track
        .codec_params
        .channels
        .map_or(2, symphonia::core::audio::Channels::count);
    let channels =
        u16::try_from(channel_count).map_err(|_| DecodeError::TooManyChannels(channel_count))?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| {
            if unsupported(&e) {
                DecodeError::UnsupportedFormat
            } else {
                DecodeError::Decode(e.to_string())
            }
        })?;

    let mut samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(e) => {
                return Err(DecodeError::Decode(e.to_string()));
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio) => interleave(&audio, &mut samples),
            Err(SymError::DecodeError(_) | SymError::ResetRequired) => {
                // Recoverable per-packet errors — skip.
            }
            Err(e) => return Err(DecodeError::Decode(e.to_string())),
        }
    }

    Ok(AudioBufferOut {
        samples,
        sample_rate,
        channels,
    })
}

/// Interleave all channels of `buffer` into `out` (f32, [-1, 1]).
///
/// Uses `AudioBufferRef::make_equivalent` to allocate a destination of the
/// right layout, then `convert` for sample-format conversion.
fn interleave(buffer: &AudioBufferRef<'_>, out: &mut Vec<f32>) {
    let mut dst: AudioBuffer<f32> = buffer.make_equivalent();
    buffer.convert(&mut dst);
    let plane_refs = dst.planes();
    let planes = plane_refs.planes();
    let frames = planes.first().map_or(0, |p| p.len());
    for frame_idx in 0..frames {
        for plane in planes {
            out.push(plane[frame_idx]);
        }
    }
}
