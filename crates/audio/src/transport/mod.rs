//! Transport state machine + playback queue + command dispatcher.
//!
//! The output sink (`cpal` wrapper in `output.rs`) is a separate concern;
//! the transport owns state + queue and dispatches commands.

mod queue;
mod state;

pub use queue::PlaybackQueue;
pub use state::TransportState;

/// Commands the front-end (or worker) can send to the transport.
///
/// `track_id` values are opaque to the transport — they're whatever the
/// caller uses to find the file to play (typically the `track.id` rowid
/// in the library DB).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportCommand {
    /// Start (or replace) the queue with a single track id and play it.
    Play(i64),
    /// Append a track id to the queue without changing transport state.
    Enqueue(i64),
    Pause,
    Resume,
    Stop,
    Next,
    Previous,
    /// Drop the queue and stop.
    ClearQueue,
}

/// Transport state + queue, updated by a stream of `TransportCommand`s.
#[derive(Debug, Default, Clone)]
pub struct Transport {
    pub state: TransportState,
    pub queue: PlaybackQueue,
}

impl Transport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a single command. The output sink is expected to be driven
    /// separately — see `Transport::current_track()`.
    #[allow(clippy::needless_pass_by_value)] // TransportCommand owns data
    pub fn dispatch(&mut self, cmd: TransportCommand) {
        match cmd {
            TransportCommand::Play(track_id) => {
                self.queue.clear();
                self.queue.push(track_id);
                self.state = self.state.play();
            }
            TransportCommand::Enqueue(track_id) => {
                self.queue.push(track_id);
            }
            TransportCommand::Pause => self.state = self.state.pause(),
            TransportCommand::Resume => self.state = self.state.resume(),
            TransportCommand::Stop => self.state = self.state.stop(),
            TransportCommand::Next => {
                self.queue.next();
            }
            TransportCommand::Previous => {
                self.queue.previous();
            }
            TransportCommand::ClearQueue => {
                self.queue.clear();
                self.state = self.state.stop();
            }
        }
    }

    pub fn current_track(&self) -> Option<i64> {
        self.queue.current()
    }
}
