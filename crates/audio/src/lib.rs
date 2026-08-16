//! Mimir audio crate.
//!
//! Tier 0 ships the decoder (P6), the transport state + queue (P7), and a
//! minimal `cpal` output device enumeration (P7, gated on the `output`
//! feature — CI has no audio device).

pub mod decode;
#[cfg(feature = "output")]
pub mod output;
pub mod transport;

pub use decode::{decode_file, AudioBufferOut as AudioBuffer, DecodeError};
pub use transport::TransportState;

#[cfg(test)]
mod tests;
