//! Transport state machine + playback queue + command dispatcher.
//!
//! The output sink (P7's `cpal` wrapper) is a separate concern; the
//! transport owns state + queue and dispatches commands.

mod queue;
mod state;

pub use queue::PlaybackQueue;
pub use state::TransportState;
