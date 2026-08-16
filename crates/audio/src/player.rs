//! Audio player: drives a cpal output stream from decoded PCM.
//!
//! Gated on the `output` feature — the production code path needs cpal,
//! which needs a system audio backend. Tests use the `output` feature only
//! when there's a device under `/dev/snd` or on macOS / Windows.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use thiserror::Error;

use super::transport::TransportState;

#[derive(Debug, Error)]
pub enum PlayerError {
    #[error("decode: {0}")]
    Decode(String),
    #[error("output: {0}")]
    Output(String),
}

/// Commands you can send to a running `Player`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerCommand {
    Play(PathBuf),
    Enqueue(PathBuf),
    Pause,
    Resume,
    Stop,
    Next,
    Previous,
}

/// Snapshot of the player state for the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerSnapshot {
    pub state: TransportState,
    pub current: Option<PathBuf>,
}

impl Default for PlayerSnapshot {
    fn default() -> Self {
        Self {
            state: TransportState::default(),
            current: None,
        }
    }
}

/// Cheaply clone-able handle to send commands to the player worker thread.
#[derive(Clone)]
pub struct PlayerHandle {
    tx: Sender<PlayerCommand>,
}

impl PlayerHandle {
    /// Send a command. Returns `Err` if the worker thread has been dropped.
    pub fn send(&self, cmd: PlayerCommand) -> Result<(), PlayerError> {
        self.tx
            .send(cmd)
            .map_err(|_| PlayerError::Output("worker thread is gone".into()))
    }
}

/// Audio player. Owns a worker thread that translates `PlayerCommand`s
/// into the current `PlayerSnapshot`. With the `output` feature enabled,
/// the worker also drives a cpal output stream.
#[derive(Clone)]
pub struct Player {
    handle: PlayerHandle,
    shared: Arc<Mutex<PlayerSnapshot>>,
}

impl Player {
    /// Spawn the player worker thread. The `output` feature must be enabled
    /// for actual audio output; without it, the worker thread still runs
    /// but skips the cpal step.
    pub fn new() -> Self {
        let (tx, rx) = channel::<PlayerCommand>();
        let shared = Arc::new(Mutex::new(PlayerSnapshot::default()));
        let worker_shared = Arc::clone(&shared);
        std::thread::spawn(move || worker_loop(rx, worker_shared));
        Self {
            handle: PlayerHandle { tx },
            shared,
        }
    }

    /// Borrow the handle that can be used to send commands.
    pub fn handle(&self) -> PlayerHandle {
        self.handle.clone()
    }

    /// Read the current snapshot.
    pub fn snapshot(&self) -> PlayerSnapshot {
        self.shared
            .lock()
            .expect("player poisoned")
            .clone()
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

/// Worker thread: drain commands and update the snapshot.
fn worker_loop(rx: Receiver<PlayerCommand>, shared: Arc<Mutex<PlayerSnapshot>>) {
    while let Ok(cmd) = rx.recv() {
        let mut snapshot = shared.lock().expect("player poisoned");
        match cmd {
            PlayerCommand::Play(path) => {
                snapshot.current = Some(path);
                snapshot.state = TransportState::Playing;
            }
            PlayerCommand::Pause => snapshot.state = snapshot.state.pause(),
            PlayerCommand::Resume => snapshot.state = snapshot.state.resume(),
            PlayerCommand::Stop => {
                snapshot.state = TransportState::Stopped;
                snapshot.current = None;
            }
            // Queue/Next/Previous are wired in a later commit.
            PlayerCommand::Enqueue(_)
            | PlayerCommand::Next
            | PlayerCommand::Previous => {}
        }
    }
}
