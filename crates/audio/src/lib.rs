//! Mimir audio crate.
//!
//! Tier 0 ships only the decoder. DSP and output land in P7.

pub mod decode;

pub use decode::{decode_file, AudioBufferOut as AudioBuffer, DecodeError};

#[cfg(test)]
mod tests;
