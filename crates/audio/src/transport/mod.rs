//! Transport state machine + playback queue + command dispatcher.
//!
//! The output sink (P7's `cpal` wrapper) is a separate concern; the
//! transport owns state + queue and dispatches commands.

mod state;

pub use state::TransportState;
