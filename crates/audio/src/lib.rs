//! Mimir audio crate.
//!
//! Tier 0 ships the decoder (P6), the transport state + queue (P7), and a
//! minimal `cpal` output device enumeration (P7, gated on the `output`
//! feature — CI has no audio device).
//!
//! P11 adds the `player` module which wires the decoder to a cpal output
//! stream, gated on the same `output` feature.

pub mod decode;
pub mod eq;
pub mod gain;
#[cfg(feature = "output")]
pub mod output;
pub mod player;
pub mod resampler;
pub mod transport;

pub use decode::{decode_file, AudioBufferOut as AudioBuffer, DecodeError};
#[cfg(feature = "output")]
pub use player::{Player, PlayerCommand, PlayerError, PlayerHandle, PlayerSnapshot};
pub use transport::{Transport, TransportCommand, TransportState};

#[cfg(test)]
mod tests;
