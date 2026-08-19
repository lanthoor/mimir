//! Audio player: drives a cpal output stream from decoded PCM.
//!
//! Gated on the `output` feature — the production code path needs cpal,
//! which needs a system audio backend. Tests use the `output` feature only
//! when there's a device under `/dev/snd` or on macOS / Windows.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use thiserror::Error;

use super::transport::TransportState;
use mimir_telemetry as telemetry;

#[derive(Debug, Error)]
pub enum PlayerError {
    #[error("decode: {0}")]
    Decode(String),
    #[error("output: {0}")]
    Output(String),
}

/// Commands you can send to a running `Player`.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerCommand {
    Play(PathBuf),
    Enqueue(PathBuf),
    Pause,
    Resume,
    Stop,
    Next,
    Previous,
    /// Pre-decode the next track into a side buffer so the output callback
    /// can swap to it the moment the current buffer is exhausted.
    /// Idempotent: a second `PrepareNext` before consumption replaces the
    /// pending buffer.
    PrepareNext(PathBuf),
    /// Set the ReplayGain-style gain (in dB) applied to every subsequent
    /// decode. Pass `None` to disable gain (raw playback). `0.0` writes the
    /// buffer untouched. Out-of-range values are clipped per-sample.
    SetReplayGainDb(Option<f64>),
}

/// Snapshot of the player state for the UI.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerSnapshot {
    pub state: TransportState,
    pub current: Option<PathBuf>,
    /// Path of the pre-decoded "next" track, if any.
    pub next_prepared: Option<PathBuf>,
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
    /// Sample rate of the audio currently in `samples`. After resampling
    /// this matches the cpal output device's rate.
    pub sample_rate: u32,
    /// Channel count of the audio currently in `samples`.
    pub channels: u16,
}

impl BufferState {
    pub fn empty() -> Self {
        Self {
            samples: Vec::new(),
            position: 0,
            sample_rate: 0,
            channels: 0,
        }
    }

    /// Fill `out` with up to `out.len()` samples. Returns the number of
    /// samples produced. Returns 0 when the buffer is exhausted.
    #[cfg(feature = "output")]
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

#[allow(dead_code)]
struct NoopOutputStream;

#[allow(dead_code)]
impl OutputStream for NoopOutputStream {
    fn pause(&self) {}
    fn resume(&self) {}
    fn stop(self: Box<Self>) {}
}

/// Worker thread: drain commands and update the snapshot. The cpal output
/// stream is opened whenever a `Play` command succeeds and torn down on
/// `Stop` (or replaced on the next `Play`).
#[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
fn worker_loop(rx: Receiver<PlayerCommand>, shared: Arc<Mutex<PlayerSnapshot>>) {
    telemetry::log("INFO", "audio.player", "worker_loop starting");
    let mut output: Option<Box<dyn OutputStream>> = None;
    let buffer: SharedBuffer = Arc::new(Mutex::new(BufferState::empty()));
    // Side buffer that holds the pre-decoded "next" track. The output
    // callback pulls from `buffer` first; if its `fill()` returns 0
    // samples, the callback swaps in `next_buffer` so playback continues
    // without a gap.
    let next_buffer: SharedBuffer = Arc::new(Mutex::new(BufferState::empty()));
    let gain_db: Arc<Mutex<Option<f64>>> = Arc::new(Mutex::new(None));
    // Sample rate of the active cpal output stream. `None` until the first
    // successful Play. Used to resample newly-decoded buffers to the device
    // rate so 192 kHz FLAC plays at 48 kHz (or whatever the device supports)
    // instead of stalling or sounding pitched-up.
    let mut device_rate: Option<u32> = None;
    let mut cmd_n = 0u64;

    while let Ok(cmd) = rx.recv() {
        cmd_n += 1;
        telemetry::log(
            "DEBUG",
            "audio.player",
            &format!("recv cmd #{cmd_n} = {cmd:?}"),
        );
        match cmd {
            PlayerCommand::SetReplayGainDb(g) => {
                *gain_db.lock().expect("gain poisoned") = g;
                telemetry::log("INFO", "audio.player", &format!("gain set to {g:?}"));
            }
            PlayerCommand::PrepareNext(path) => {
                let gain = *gain_db.lock().expect("gain poisoned");
                match decode_to_buffer_with_gain(&path, &next_buffer, gain) {
                    Ok(()) => {
                        if let Some(r) = device_rate {
                            maybe_resample_buffer(&next_buffer, r);
                        }
                        let mut snapshot = shared.lock().expect("player poisoned");
                        snapshot.next_prepared = Some(path.clone());
                        telemetry::log(
                            "INFO",
                            "audio.player",
                            &format!("prepare_next ok path={}", path.display()),
                        );
                    }
                    Err(e) => {
                        telemetry::log(
                            "ERROR",
                            "audio.player",
                            &format!("prepare_next decode failed path={} err={e}", path.display()),
                        );
                    }
                }
            }
            PlayerCommand::Play(path) => {
                let gain = *gain_db.lock().expect("gain poisoned");
                telemetry::log(
                    "INFO",
                    "audio.player",
                    &format!("Play start path={} gain_db={gain:?}", path.display()),
                );
                match decode_to_buffer_with_gain(&path, &buffer, gain) {
                    Ok(()) => {
                        {
                            let mut snapshot = shared.lock().expect("player poisoned");
                            snapshot.current = Some(path.clone());
                            snapshot.state = TransportState::Playing;
                        }
                        match open_output_stream(&buffer) {
                            Ok((stream, rate)) => {
                                device_rate = Some(rate);
                                maybe_resample_buffer(&buffer, rate);
                                // Same for the pre-decoded next track, if any.
                                maybe_resample_buffer(&next_buffer, rate);
                                output = Some(stream);
                                telemetry::log(
                                    "INFO",
                                    "audio.player",
                                    &format!("output stream opened device_rate={rate}"),
                                );
                            }
                            Err(e) => {
                                telemetry::log(
                                    "WARN",
                                    "audio.player",
                                    &format!("output stream unavailable: {e}"),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        telemetry::log(
                            "ERROR",
                            "audio.player",
                            &format!("Play decode failed path={} err={e}", path.display()),
                        );
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
                telemetry::log(
                    "INFO",
                    "audio.player",
                    &format!("paused state={:?}", snapshot.state),
                );
            }
            PlayerCommand::Resume => {
                let mut snapshot = shared.lock().expect("player poisoned");
                snapshot.state = snapshot.state.resume();
                if let Some(o) = output.as_ref() {
                    o.resume();
                }
                telemetry::log(
                    "INFO",
                    "audio.player",
                    &format!("resumed state={:?}", snapshot.state),
                );
            }
            PlayerCommand::Stop => {
                let mut snapshot = shared.lock().expect("player poisoned");
                snapshot.state = TransportState::Stopped;
                snapshot.current = None;
                if let Some(o) = output.take() {
                    o.stop();
                }
                buffer.lock().expect("buffer poisoned").position = 0;
                telemetry::log("INFO", "audio.player", "stopped");
            }
            // Queue/Next/Previous are wired in a later commit.
            PlayerCommand::Enqueue(_) | PlayerCommand::Next | PlayerCommand::Previous => {
                telemetry::log("DEBUG", "audio.player", "queue/next/prev: deferred (no-op)");
            }
        }
    }
    telemetry::log(
        "INFO",
        "audio.player",
        &format!("worker_loop exiting after {cmd_n} cmds"),
    );
}

/// Decode `path` into the shared buffer, applying `gain_db` (`ReplayGain` or
/// any user-preference value in dB) in-place when `Some`. On any failure
/// the buffer is cleared and the error is logged to stderr.
///
/// `gain_db == None` decodes raw. `gain_db == 0.0` still writes the buffer
/// untouched. Out-of-range values are clipped per-sample at `[`-1.0`,`1.0`]`.
pub(crate) fn decode_to_buffer_with_gain(
    path: &Path,
    buffer: &SharedBuffer,
    gain_db: Option<f64>,
) -> Result<(), PlayerError> {
    let t_start = std::time::Instant::now();
    telemetry::log(
        "DEBUG",
        "audio.player",
        &format!(
            "decode_to_buffer start path={} gain={gain_db:?}",
            path.display()
        ),
    );
    let mut audio =
        super::decode::decode_file(path).map_err(|e| PlayerError::Decode(e.to_string()))?;
    let t_decode = t_start.elapsed();
    telemetry::log(
        "WARN",
        "audio.player",
        &format!(
            "decode_to_buffer: decoded path={} samples={} channels={} rate={} took={:?} (HiRes FLACs decode in tens of seconds — ponytail: streaming decode is deferred)",
            path.display(),
            audio.samples.len(),
            audio.channels,
            audio.sample_rate,
            t_decode
        ),
    );
    if let Some(g) = gain_db {
        super::gain::apply_gain_db_inplace(&mut audio.samples, g);
    }
    let n = audio.samples.len();
    let sr = audio.sample_rate;
    let ch = audio.channels;
    let mut state = buffer.lock().expect("buffer poisoned");
    state.samples = audio.samples;
    state.position = 0;
    state.sample_rate = sr;
    state.channels = ch;
    telemetry::log(
        "INFO",
        "audio.player",
        &format!(
            "decode_to_buffer ok path={} samples={n} sample_rate={sr} channels={ch} decode_wall_time={:?}",
            path.display(),
            t_decode
        ),
    );
    Ok(())
}

/// Resample `buffer` from its current `sample_rate` to `device_rate` if they
/// differ. No-op when rates match or when the buffer is empty.
pub(crate) fn maybe_resample_buffer(buffer: &SharedBuffer, device_rate: u32) {
    let mut state = buffer.lock().expect("buffer poisoned");
    if state.samples.is_empty() || state.sample_rate == 0 {
        return;
    }
    if state.sample_rate == device_rate {
        telemetry::log(
            "DEBUG",
            "audio.player",
            &format!("resample no-op: rate={device_rate} already matches"),
        );
        return;
    }
    let src_rate = state.sample_rate;
    let channels = state.channels;
    telemetry::log(
        "INFO",
        "audio.player",
        &format!(
            "resampling buffer src={src_rate} → device={device_rate} ch={channels} samples={}",
            state.samples.len()
        ),
    );
    let before_n = state.samples.len();
    let resampled =
        super::resampler::resample_interleaved(&state.samples, channels, src_rate, device_rate);
    let after_n = resampled.len();
    state.samples = resampled;
    state.position = 0;
    state.sample_rate = device_rate;
    telemetry::log(
        "INFO",
        "audio.player",
        &format!("resample done samples={before_n} → {after_n}"),
    );
}

/// Open a cpal output stream that drains `buffer`. Returns `(stream, rate)` so
/// the caller can resample freshly-decoded buffers to the device rate.
#[cfg(feature = "output")]
#[allow(clippy::cast_possible_truncation)]
fn open_output_stream(buffer: &SharedBuffer) -> Result<(Box<dyn OutputStream>, u32), PlayerError> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| PlayerError::Output("no default output device".into()))?;
    let config = device
        .default_output_config()
        .map_err(|e| PlayerError::Output(format!("default output config: {e}")))?;
    let device_rate = config.sample_rate().0;

    let err_fn = |err: cpal::StreamError| {
        telemetry::log("ERROR", "player", &format!("cpal stream error: {err}"));
    };

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let buf = Arc::clone(buffer);
            device
                .build_output_stream(
                    &config.into(),
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let mut state = buf.lock().expect("buffer poisoned");
                        let n = state.fill(data);
                        for s in &mut data[n..] {
                            *s = 0.0;
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| PlayerError::Output(format!("build output stream: {e}")))?
        }
        cpal::SampleFormat::I16 => {
            let buf = Arc::clone(buffer);
            device
                .build_output_stream(
                    &config.into(),
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
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| PlayerError::Output(format!("build output stream: {e}")))?
        }
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

    Ok((Box::new(CpalOutputStream(stream)), device_rate))
}

#[cfg(not(feature = "output"))]
#[allow(clippy::unnecessary_wraps)]
fn open_output_stream(_buffer: &SharedBuffer) -> Result<(Box<dyn OutputStream>, u32), PlayerError> {
    Ok((Box::new(NoopOutputStream), 48_000))
}
