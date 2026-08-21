//! Mimir audio crate.
//!
//! Tier 0 ships the decoder (P6), the transport state + queue (P7).
//!
//! P11 adds the `player` module which wires the decoder to a rodio
//! playback queue, gated on the `output` feature. Rodio handles device
//! open, ring buffering, and sample-rate conversion internally.

pub mod decode;
pub mod eq;
pub mod gain;
pub mod player;
pub mod transport;

pub use decode::{decode_file, AudioBufferOut as AudioBuffer, DecodeError};
#[cfg(feature = "output")]
pub use player::{Player, PlayerCommand, PlayerError, PlayerHandle, PlayerSnapshot};
pub use transport::{Transport, TransportCommand, TransportState};

#[cfg(test)]
mod tests;
