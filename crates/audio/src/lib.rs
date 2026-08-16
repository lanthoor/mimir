//! Mimir audio crate.
//!
//! Tier 0 ships the decoder (P6) and the transport state + queue (P7).
//! Output sink (cpal) is stubbed behind a feature flag — the CI runner has
//! no audio device.

pub mod decode;
pub mod transport;

pub use decode::{decode_file, AudioBufferOut as AudioBuffer, DecodeError};
pub use transport::{PlaybackQueue, Transport, TransportCommand, TransportState};

#[cfg(test)]
mod tests;
