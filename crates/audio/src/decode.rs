//! Decoding audio files to interleaved f32 PCM via `symphonia`.
//!
//! The decoder is intentionally minimal: one pass through the file, no
//! seeking, no resampling. DSP and output wiring happen in P7.

use std::path::Path;

use mimir_telemetry as telemetry;
use symphonia::core::audio::Channels;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
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
/// Decodes the *default audio track* in the container to completion. For Tier
/// 0's single-track-per-file assumption this is sufficient.
#[allow(clippy::too_many_lines)]
pub fn decode_file(path: &Path) -> Result<AudioBufferOut, DecodeError> {
    telemetry::log(
        "INFO",
        "audio.decode",
        &format!("start path={}", path.display()),
    );
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());

    let mut format = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            mss,
            FormatOptions::default(),
            symphonia::core::meta::MetadataOptions::default(),
        )
        .map_err(|e| {
            telemetry::log(
                "ERROR",
                "audio.decode",
                &format!("probe failed path={} err={e}", path.display()),
            );
            if unsupported(&e) {
                DecodeError::UnsupportedFormat
            } else {
                DecodeError::Decode(e.to_string())
            }
        })?;

    let track = format.default_track(TrackType::Audio).ok_or_else(|| {
        telemetry::log(
            "WARN",
            "audio.decode",
            &format!("no audio tracks in {}", path.display()),
        );
        DecodeError::NoTracks
    })?;

    // symphonia 0.6: `track.codec_params` is now `Option<CodecParameters>`,
    // `default_track(TrackType::Audio)` guarantees an audio codec, so this
    // extraction is the "no audio track" case rather than a codec mismatch.
    let audio = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| {
            telemetry::log(
                "WARN",
                "audio.decode",
                &format!("no audio codec params in {}", path.display()),
            );
            DecodeError::NoTracks
        })?;

    let track_id = track.id;
    let sample_rate = audio.sample_rate.unwrap_or(44_100);
    let channel_count = audio.channels.as_ref().map_or(2, Channels::count);
    let channels =
        u16::try_from(channel_count).map_err(|_| DecodeError::TooManyChannels(channel_count))?;
    telemetry::log(
        "DEBUG",
        "audio.decode",
        &format!("track id={track_id} sample_rate={sample_rate} channels={channel_count}"),
    );

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(audio, &AudioDecoderOptions::default())
        .map_err(|e| {
            telemetry::log(
                "ERROR",
                "audio.decode",
                &format!("decoder init failed path={} err={e}", path.display()),
            );
            if unsupported(&e) {
                DecodeError::UnsupportedFormat
            } else {
                DecodeError::Decode(e.to_string())
            }
        })?;

    let mut samples: Vec<f32> = Vec::new();
    let mut packets = 0u64;
    let mut decoder_errors = 0u64;

    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => {
                telemetry::log(
                    "DEBUG",
                    "audio.decode",
                    &format!("EOF reached packets={packets} samples={}", samples.len()),
                );
                break;
            }
            Err(SymError::IoError(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                telemetry::log(
                    "DEBUG",
                    "audio.decode",
                    &format!("EOF reached packets={packets} samples={}", samples.len()),
                );
                break;
            }
            Err(SymError::ResetRequired) => {
                decoder_errors += 1;
                telemetry::log(
                    "WARN",
                    "audio.decode",
                    &format!(
                        "track list changed, stopping at packets={packets} samples={}",
                        samples.len()
                    ),
                );
                break;
            }
            Err(e) => {
                telemetry::log(
                    "ERROR",
                    "audio.decode",
                    &format!("next_packet err path={} err={e}", path.display()),
                );
                return Err(DecodeError::Decode(e.to_string()));
            }
        };

        if packet.track_id != track_id {
            continue;
        }

        packets += 1;
        match decoder.decode(&packet) {
            Ok(audio_buf) => audio_buf.copy_to_vec_interleaved(&mut samples),
            Err(SymError::DecodeError(_)) => {
                decoder_errors += 1;
                telemetry::log(
                    "WARN",
                    "audio.decode",
                    &format!(
                        "recoverable decoder err n={decoder_errors} samples={}",
                        samples.len()
                    ),
                );
            }
            Err(e) => return Err(DecodeError::Decode(e.to_string())),
        }
    }

    telemetry::log(
        "INFO",
        "audio.decode",
        &format!(
            "done path={} packets={packets} samples={} decoder_errors={decoder_errors}",
            path.display(),
            samples.len()
        ),
    );
    Ok(AudioBufferOut {
        samples,
        sample_rate,
        channels,
    })
}
