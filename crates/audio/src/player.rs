//! Audio player: drives a rodio playback queue from a streaming decoder.
//!
//! Rodio handles the device open, the cpal callback, the ring buffer, the
//! on-the-fly sample-rate conversion, *and* per-packet decode. The worker
//! no longer pre-resamples the whole decoded buffer before opening the
//! output stream, and no longer decodes the file into memory — opening a
//! `BufReader<File>` and handing rodio a `Decoder` is enough; rodio pulls
//! packets from the file as the audio thread needs them. First-play and
//! post-stop/post-pause latency drop to a file open + format probe
//! (milliseconds), not a full-file decode (tens of seconds on `HiRes` FLACs).
//!
//! Gated on the `output` feature — the production code path needs an
//! audio backend. Tests use the `output` feature only when there's a device
//! under `/dev/snd` or on macOS / Windows.

// ponytail: `rodio_sink` in `worker_loop` is bound but never read —
// dropping the MixerDeviceSink stops playback, so we hold it for its
// Drop side-effect. Allow the lint module-wide to avoid a one-off at
// the assignment site.
#![cfg_attr(feature = "output", allow(unused_assignments, unused_variables))]

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use rodio::Player as RodioPlayer;
use rodio::Source;
use thiserror::Error;

use super::transport::TransportState;
use mimir_telemetry as telemetry;

/// Decoder type the worker hands to rodio. `Decoder<File>` via `TryFrom<File>`
/// gives `Decoder<BufReader<File>>` with `byte_len` populated for accurate
/// duration / seek support.
type StreamingSource = rodio::Decoder<BufReader<File>>;

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
    /// Record the next track's path so the worker can open it the moment
    /// the current source ends, for gapless handoff. A second `PrepareNext`
    /// before consumption replaces the pending path.
    PrepareNext(PathBuf),
    /// Set the ReplayGain-style gain (in dB) applied to every subsequent
    /// playback via rodio's volume control. Pass `None` to disable gain
    /// (raw playback, volume = 1.0). `0.0` sets volume to 1.0.
    SetReplayGainDb(Option<f64>),
}

/// Snapshot of the player state for the UI.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerSnapshot {
    pub state: TransportState,
    pub current: Option<PathBuf>,
    /// Path of the prepared "next" track, if any.
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
/// the worker also drives a rodio playback queue.
#[derive(Clone)]
pub struct Player {
    handle: PlayerHandle,
    shared: Arc<Mutex<PlayerSnapshot>>,
}

impl Player {
    /// Spawn the player worker thread. The `output` feature must be enabled
    /// for actual audio output; without it, the worker thread still runs
    /// but skips the rodio step.
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

/// `ReplayGain` helper.
fn replay_gain_to_volume(gain_db: Option<f64>) -> f32 {
    match gain_db {
        None => 1.0,
        Some(d) => super::gain::db_to_linear(d).clamp(0.0, 8.0),
    }
}

/// Worker thread: drain commands and update the snapshot. A rodio
/// `Player` is attached on the first `Play` and reused across subsequent
/// tracks.
#[allow(clippy::too_many_lines)]
fn worker_loop(rx: Receiver<PlayerCommand>, shared: Arc<Mutex<PlayerSnapshot>>) {
    telemetry::log("INFO", "audio.player", "worker_loop starting");
    // ponytail: `rodio_sink` is never read — it exists only to keep the
    // MixerDeviceSink alive. Dropping it stops playback, so it must outlive
    // every RodioPlayer created from its mixer.
    let mut rodio_sink: Option<rodio::MixerDeviceSink> = None;
    let mut rodio_player: Option<RodioPlayer> = None;
    let mut gain_db: Option<f64> = None;
    // Path of the track prepared for gapless handoff. Decoder<File> is not
    // Clone, so we re-open the file when the worker actually needs the
    // source. Stash the path, not the decoder.
    let mut pending_next: Option<PathBuf> = None;
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
                gain_db = g;
                if let Some(p) = rodio_player.as_ref() {
                    p.set_volume(replay_gain_to_volume(g));
                }
                telemetry::log("INFO", "audio.player", &format!("gain set to {g:?}"));
            }
            PlayerCommand::PrepareNext(path) => {
                pending_next = Some(path.clone());
                // Fast-lane: if a player is already up, open the file now
                // and queue the source so playback is gapless.
                if let Some(p) = rodio_player.as_ref() {
                    match open_streaming_source(&path) {
                        Ok(src) => p.append(src),
                        Err(e) => telemetry::log(
                            "ERROR",
                            "audio.player",
                            &format!("prepare_next open failed path={} err={e}", path.display()),
                        ),
                    }
                }
                let mut snapshot = shared.lock().expect("player poisoned");
                snapshot.next_prepared = Some(path.clone());
                telemetry::log(
                    "INFO",
                    "audio.player",
                    &format!("prepare_next ok path={}", path.display()),
                );
            }
            PlayerCommand::Play(path) => {
                telemetry::log(
                    "INFO",
                    "audio.player",
                    &format!("Play start path={} gain_db={gain_db:?}", path.display()),
                );
                match open_streaming_source(&path) {
                    Ok(source) => {
                        let pending = pending_next.take();
                        match rodio_player.as_mut() {
                            Some(p) => {
                                p.stop();
                                p.set_volume(replay_gain_to_volume(gain_db));
                                p.append(source);
                                if let Some(next_path) = pending.as_ref() {
                                    match open_streaming_source(next_path) {
                                        Ok(next_src) => p.append(next_src),
                                        Err(e) => telemetry::log(
                                            "ERROR",
                                            "audio.player",
                                            &format!(
                                                "Play: pending-next open failed path={} err={e}",
                                                next_path.display()
                                            ),
                                        ),
                                    }
                                }
                                p.play();
                            }
                            None => match open_first_player(source, gain_db) {
                                Ok((sink, p)) => {
                                    if let Some(next_path) = pending.as_ref() {
                                        match open_streaming_source(next_path) {
                                            Ok(next_src) => p.append(next_src),
                                            Err(e) => telemetry::log(
                                                "ERROR",
                                                "audio.player",
                                                &format!(
                                                    "Play: pending-next open failed path={} err={e}",
                                                    next_path.display()
                                                ),
                                            ),
                                        }
                                    }
                                    rodio_sink = Some(sink);
                                    rodio_player = Some(p);
                                }
                                Err(e) => {
                                    telemetry::log(
                                        "ERROR",
                                        "audio.player",
                                        &format!(
                                            "Play output open failed path={} err={e}",
                                            path.display()
                                        ),
                                    );
                                    let mut snapshot = shared.lock().expect("player poisoned");
                                    snapshot.current = Some(path);
                                    snapshot.state = TransportState::Stopped;
                                    continue;
                                }
                            },
                        }
                        {
                            let mut snapshot = shared.lock().expect("player poisoned");
                            snapshot.current = Some(path.clone());
                            snapshot.state = TransportState::Playing;
                            snapshot.next_prepared = None;
                        }
                        telemetry::log(
                            "INFO",
                            "audio.player",
                            &format!("Play ok path={}", path.display()),
                        );
                    }
                    Err(e) => {
                        telemetry::log(
                            "ERROR",
                            "audio.player",
                            &format!("Play open failed path={} err={e}", path.display()),
                        );
                        let mut snapshot = shared.lock().expect("player poisoned");
                        snapshot.current = Some(path);
                        snapshot.state = TransportState::Stopped;
                    }
                }
            }
            PlayerCommand::Pause => {
                if let Some(p) = rodio_player.as_ref() {
                    p.pause();
                }
                let mut snapshot = shared.lock().expect("player poisoned");
                snapshot.state = snapshot.state.pause();
                telemetry::log(
                    "INFO",
                    "audio.player",
                    &format!("paused state={:?}", snapshot.state),
                );
            }
            PlayerCommand::Resume => {
                if let Some(p) = rodio_player.as_ref() {
                    p.play();
                }
                let mut snapshot = shared.lock().expect("player poisoned");
                snapshot.state = snapshot.state.resume();
                telemetry::log(
                    "INFO",
                    "audio.player",
                    &format!("resumed state={:?}", snapshot.state),
                );
            }
            PlayerCommand::Stop => {
                if let Some(p) = rodio_player.as_ref() {
                    p.stop();
                }
                pending_next = None;
                let mut snapshot = shared.lock().expect("player poisoned");
                snapshot.state = TransportState::Stopped;
                snapshot.current = None;
                snapshot.next_prepared = None;
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

/// Open `path` and return a rodio `Decoder` ready for `Player::append`.
/// Rodio pulls packets from the file on demand — no whole-file decode.
fn open_streaming_source(path: &Path) -> Result<StreamingSource, PlayerError> {
    let t_start = std::time::Instant::now();
    let file = File::open(path).map_err(|e| PlayerError::Decode(e.to_string()))?;
    let source = StreamingSource::try_from(file).map_err(|e| PlayerError::Decode(e.to_string()))?;
    telemetry::log(
        "INFO",
        "audio.player",
        &format!(
            "open_streaming_source ok path={} ch={} rate={} took={:?}",
            path.display(),
            source.channels().get(),
            source.sample_rate().get(),
            t_start.elapsed()
        ),
    );
    Ok(source)
}

/// Open the first rodio `Player` for a fresh playback session. Returns
/// `(sink, player)`: the `MixerDeviceSink` owns the OS device and must be
/// kept alive alongside the `Player` for playback to continue.
#[cfg(feature = "output")]
fn open_first_player(
    source: StreamingSource,
    gain_db: Option<f64>,
) -> Result<(rodio::MixerDeviceSink, RodioPlayer), PlayerError> {
    let sink = rodio::DeviceSinkBuilder::open_default_sink()
        .map_err(|e| PlayerError::Output(format!("open default audio sink: {e}")))?;
    let player = RodioPlayer::connect_new(sink.mixer());
    player.set_volume(replay_gain_to_volume(gain_db));
    player.append(source);
    player.play();
    Ok((sink, player))
}

#[cfg(not(feature = "output"))]
fn open_first_player(
    _source: StreamingSource,
    _gain_db: Option<f64>,
) -> Result<(rodio::MixerDeviceSink, RodioPlayer), PlayerError> {
    Err(PlayerError::Output(
        "output feature disabled at compile time".into(),
    ))
}
