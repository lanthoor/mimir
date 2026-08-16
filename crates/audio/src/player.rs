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
        self.shared.lock().expect("player poisoned").clone()
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared audio buffer + read position. The cpal callback pulls samples
/// from this in chunks. Wrapped in `Arc<Mutex<_>>` so the callback can
/// borrow without taking ownership of the buffer.
pub(crate) type SharedBuffer = Arc<Mutex<BufferState>>;

pub(crate) struct BufferState {
    pub samples: Vec<f32>,
    pub position: usize,
}

impl BufferState {
    pub fn empty() -> Self {
        Self {
            samples: Vec::new(),
            position: 0,
        }
    }

    /// Fill `out` with up to `out.len()` samples. Returns the number of
    /// samples produced. Returns 0 when the buffer is exhausted.
    pub fn fill(&mut self, out: &mut [f32]) -> usize {
        let remaining = self.samples.len().saturating_sub(self.position);
        let n = remaining.min(out.len());
        if n == 0 {
            return 0;
        }
        out[..n].copy_from_slice(&self.samples[self.position..self.position + n]);
        self.position += n;
        n
    }
}

/// Trait the worker uses to control the active output stream. Implemented
/// by `CpalOutputStream` when the `output` feature is on, and by
/// `NoopOutputStream` otherwise.
trait OutputStream {
    fn pause(&self);
    fn resume(&self);
    fn stop(self: Box<Self>);
}

#[cfg(feature = "output")]
struct CpalOutputStream(cpal::Stream);

#[cfg(feature = "output")]
impl OutputStream for CpalOutputStream {
    fn pause(&self) {
        use cpal::traits::StreamTrait;
        self.0.pause().ok();
    }
    fn resume(&self) {
        use cpal::traits::StreamTrait;
        self.0.play().ok();
    }
    fn stop(self: Box<Self>) {
        use cpal::traits::StreamTrait;
        self.0.pause().ok();
    }
}

struct NoopOutputStream;

impl OutputStream for NoopOutputStream {
    fn pause(&self) {}
    fn resume(&self) {}
    fn stop(self: Box<Self>) {}
}

/// Worker thread: drain commands and update the snapshot. The cpal output
/// stream is opened whenever a `Play` command succeeds and torn down on
/// `Stop` (or replaced on the next `Play`).
fn worker_loop(rx: Receiver<PlayerCommand>, shared: Arc<Mutex<PlayerSnapshot>>) {
    let mut output: Option<Box<dyn OutputStream>> = None;
    let buffer: SharedBuffer = Arc::new(Mutex::new(BufferState::empty()));

    while let Ok(cmd) = rx.recv() {
        match cmd {
            PlayerCommand::Play(path) => {
                match decode_to_buffer(&path, &buffer) {
                    Ok(_) => {
                        {
                            let mut snapshot = shared.lock().expect("player poisoned");
                            snapshot.current = Some(path.clone());
                            snapshot.state = TransportState::Playing;
                        }
                        output = open_output_stream(&buffer).ok();
                    }
                    Err(_e) => {
                        let mut snapshot = shared.lock().expect("player poisoned");
                        snapshot.current = Some(path);
                        snapshot.state = TransportState::Stopped;
                    }
                }
            }
            PlayerCommand::Pause => {
                let mut snapshot = shared.lock().expect("player poisoned");
                snapshot.state = snapshot.state.pause();
                if let Some(o) = output.as_ref() {
                    o.pause();
                }
            }
            PlayerCommand::Resume => {
                let mut snapshot = shared.lock().expect("player poisoned");
                snapshot.state = snapshot.state.resume();
                if let Some(o) = output.as_ref() {
                    o.resume();
                }
            }
            PlayerCommand::Stop => {
                let mut snapshot = shared.lock().expect("player poisoned");
                snapshot.state = TransportState::Stopped;
                snapshot.current = None;
                if let Some(o) = output.take() {
                    o.stop();
                }
                buffer.lock().expect("buffer poisoned").position = 0;
            }
            // Queue/Next/Previous are wired in a later commit.
            PlayerCommand::Enqueue(_)
            | PlayerCommand::Next
            | PlayerCommand::Previous => {}
        }
    }
}

/// Decode `path` into the shared buffer. On any failure the buffer is
/// cleared and the error is logged to stderr.
fn decode_to_buffer(path: &PathBuf, buffer: &SharedBuffer) -> Result<(), PlayerError> {
    let audio = super::decode::decode_file(path)
        .map_err(|e| PlayerError::Decode(e.to_string()))?;
    let mut state = buffer.lock().expect("buffer poisoned");
    state.samples = audio.samples;
    state.position = 0;
    Ok(())
}

/// Open a cpal output stream that drains `buffer`. Returns `None` when no
/// output device is available (so the worker reaches the end of the song
/// without crashing).
#[cfg(feature = "output")]
fn open_output_stream(buffer: &SharedBuffer) -> Result<Box<dyn OutputStream>, PlayerError> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| PlayerError::Output("no default output device".into()))?;
    let config = device
        .default_output_config()
        .map_err(|e| PlayerError::Output(format!("default output config: {e}")))?;

    let err_fn = |err: cpal::StreamError| eprintln!("cpal stream error: {err}");

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device
            .build_output_stream(
                &config.into(),
                {
                    let buf = Arc::clone(buffer);
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let mut state = buf.lock().expect("buffer poisoned");
                        let n = state.fill(data);
                        for s in &mut data[n..] {
                            *s = 0.0;
                        }
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| PlayerError::Output(format!("build output stream: {e}")))?,
        cpal::SampleFormat::I16 => device
            .build_output_stream(
                &config.into(),
                {
                    let buf = Arc::clone(buffer);
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        let mut state = buf.lock().expect("buffer poisoned");
                        let mut tmp = vec![0.0f32; data.len()];
                        let n = state.fill(&mut tmp);
                        for (i, s) in data.iter_mut().enumerate() {
                            *s = if i < n {
                                (tmp[i].clamp(-1.0, 1.0) * 32_768.0) as i16
                            } else {
                                0
                            };
                        }
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| PlayerError::Output(format!("build output stream: {e}")))?,
        // Other formats not supported in P11 — explicit error rather than a
        // silent fallback so the CI log shows the gap.
        other => {
            return Err(PlayerError::Output(format!(
                "unsupported sample format: {other:?}"
            )));
        }
    };

    stream
        .play()
        .map_err(|e| PlayerError::Output(format!("stream play: {e}")))?;

    Ok(Box::new(CpalOutputStream(stream)))
}

#[cfg(not(feature = "output"))]
fn open_output_stream(_buffer: &SharedBuffer) -> Result<Box<dyn OutputStream>, PlayerError> {
    Ok(Box::new(NoopOutputStream))
}
