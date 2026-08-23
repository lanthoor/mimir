//! Audio player: a persistent playlist + a playhead, driven via rodio.
//!
//! Rodio opens the device, owns the callback + ring buffer + sample-rate
//! conversion, and pulls packets from the file on demand — the worker hands
//! it a `Decoder<BufReader<File>>` per track (a file open + format probe, not
//! a whole-file pre-decode).
//!
//! This worker owns a **persistent list** of tracks and a **playhead**
//! (the index that's playing). `Next`/`Previous` step the playhead around the
//! list — past the end it wraps to the start (a circular playlist). Nothing
//! is *consumed*: playing a track or ending it never removes it; `Stop` just
//! halts playback and leaves the list + playhead intact. Only an explicit
//! `RemoveAt` / `Move` / `ClearQueue` mutates the list. Per-track replay gain
//! is applied via the player volume at each start so a track change swaps the
//! level cleanly.
//!
//! Gated on the `output` feature — the production code path needs an audio
//! backend. Tests skip the device-dependent paths when there's no device.

#![allow(unused_assignments, unused_variables)]
// ponytail: `rodio_sink` in `worker_loop` is bound but never read —
// dropping the MixerDeviceSink stops playback, so we hold it for its Drop.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::Player as RodioPlayer;
use rodio::Source;
use thiserror::Error;

use super::transport::TransportState;
use mimir_telemetry as telemetry;

/// Decoder type the worker hands to rodio. `Decoder<File>` via `TryFrom<File>`
/// gives `Decoder<BufReader<File>>` with `byte_len` populated for accurate
/// duration / seek support.
type StreamingSource = rodio::Decoder<BufReader<File>>;

/// One track in the playlist: path + optional per-track replay gain (dB;
/// `None` plays raw).
pub type QueueTrack = (PathBuf, Option<f64>);

#[derive(Debug, Error)]
pub enum PlayerError {
    #[error("decode: {0}")]
    Decode(String),
    #[error("output: {0}")]
    Output(String),
}

/// Commands you can send to a running `Player`.
///
/// Indices (`PlayAt` / `RemoveAt` / `Move`) are positions in the playlist —
/// 0 is the first track, `len-1` the last.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerCommand {
    /// Play a single track: replace the playlist with just this one and
    /// start it (playhead 0).
    Play(PathBuf, Option<f64>),
    /// Append a track to the end of the playlist. Does not change playback;
    /// if the playlist was empty it becomes the playhead (but does not
    /// auto-play).
    Enqueue(PathBuf, Option<f64>),
    /// Replace the playlist and start `first`, with `rest` in order behind
    /// it (playhead 0). The atomic "play this folder/album" form.
    PlayAndEnqueue {
        first: PathBuf,
        gain: Option<f64>,
        rest: Vec<QueueTrack>,
    },
    /// Advance the playhead by one (wrapping to track 0 after the last) and
    /// start that track.
    Next,
    /// Step the playhead back by one (wrapping to the last at track 0) and
    /// start that track.
    Previous,
    /// Start the track at playlist position `i`, setting the playhead there.
    PlayAt(usize),
    /// Remove a track from the playlist by position; the playhead is kept
    /// coherent (removing the current one advances to the next).
    RemoveAt(usize),
    /// Move a playlist entry from `from` to `to` (splice, post-removal
    /// positions). The playhead follows the played track. No-op when
    /// `from`/`to` are out of range or equal.
    Move {
        from: usize,
        to: usize,
    },
    /// Empty the playlist, drop the playhead, and stop.
    ClearQueue,
    Pause,
    Resume,
    /// Halt playback. The playlist + playhead are preserved (unlike the old
    /// consuming queue) — `Next` picks up where it left off.
    Stop,
}

/// Snapshot of the player state for the UI.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerSnapshot {
    pub state: TransportState,
    /// Currently playing track (the one at the playhead), if any.
    pub current: Option<PathBuf>,
    /// Playback position within the current track, seconds.
    pub position_secs: f32,
    /// Total duration of the current track, seconds (0 when unknown).
    pub total_secs: f32,
}

/// The playlist exposed for reads: the tracks in order plus which one is the
/// playhead (`None` when nothing is selected).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct QueueView {
    pub tracks: Vec<PathBuf>,
    pub playhead: Option<usize>,
}

/// Worker-side copy of the playlist the worker publishes after every
/// mutation so other threads read the authoritative order. `ready` flips
/// once the command loop is up.
struct WorkerQueue {
    ready: bool,
    tracks: Vec<QueueTrack>,
    playhead: Option<usize>,
}

impl WorkerQueue {
    fn new() -> Self {
        Self {
            ready: false,
            tracks: Vec::new(),
            playhead: None,
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

/// Audio player. Owns a worker thread that translates `PlayerCommand`s into
/// the current `PlayerSnapshot` and drives rodio.
#[derive(Clone)]
pub struct Player {
    handle: PlayerHandle,
    shared: Arc<Mutex<PlayerSnapshot>>,
    worker_queue: Arc<Mutex<WorkerQueue>>,
}

impl Player {
    /// Spawn the player worker thread. The `output` feature must be enabled
    /// for actual audio output — the whole module is feature-gated for that.
    pub fn new() -> Self {
        let (tx, rx) = channel::<PlayerCommand>();
        let shared = Arc::new(Mutex::new(PlayerSnapshot::default()));
        let worker_queue = Arc::new(Mutex::new(WorkerQueue::new()));
        let worker_shared = Arc::clone(&shared);
        let worker_wq = Arc::clone(&worker_queue);
        std::thread::Builder::new()
            .name("mimir-player".into())
            .spawn(move || worker_loop(rx, worker_shared, worker_wq))
            .expect("spawn player worker");
        Self {
            handle: PlayerHandle { tx },
            shared,
            worker_queue,
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

    /// The playlist + playhead as the worker holds it. Waits (up to 200 ms)
    /// for the worker loop to come up so the first read is not racy.
    pub fn queue_view(&self) -> QueueView {
        let guard = self.worker_queue_wait();
        let tracks = guard.tracks.iter().map(|(p, _)| p.clone()).collect();
        QueueView {
            tracks,
            playhead: guard.playhead,
        }
    }

    fn worker_queue_wait(&self) -> std::sync::MutexGuard<'_, WorkerQueue> {
        let deadline = std::time::Instant::now() + Duration::from_millis(200);
        loop {
            let guard = self.worker_queue.lock().expect("worker queue poisoned");
            if guard.ready || std::time::Instant::now() >= deadline {
                return guard;
            }
            drop(guard);
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

/// `ReplayGain` helper: dB → linear gain, clamped so +20 dB is the ceiling
/// (≈ 10×; plenty loud, avoids runaway values in tags).
fn replay_gain_to_volume(gain_db: Option<f64>) -> f32 {
    match gain_db {
        None => 1.0,
        Some(d) => super::gain::db_to_linear(d).clamp(0.0, 8.0),
    }
}

/// Open a track and start it on `player` (creating the device sink if first).
/// Returns `Some(total_secs)` on success, `None` on failure (snapshot set to
/// Stopped on failure too).
fn start_track(
    player: &mut Option<RodioPlayer>,
    sink: &mut Option<rodio::MixerDeviceSink>,
    path: &Path,
    gain_db: Option<f64>,
    shared: &Arc<Mutex<PlayerSnapshot>>,
) -> Option<f32> {
    let source = match open_streaming_source(path) {
        Ok(s) => s,
        Err(e) => {
            telemetry::log(
                "ERROR",
                "audio.player",
                &format!("start open failed path={} err={e}", path.display()),
            );
            let mut snapshot = shared.lock().expect("player poisoned");
            snapshot.state = TransportState::Stopped;
            snapshot.current = Some(path.to_path_buf());
            return None;
        }
    };
    let total_secs = source
        .total_duration()
        .map(|d| d.as_secs_f32())
        .unwrap_or_default();
    if let Some(p) = player.as_mut() {
        // stop() drops in-flight sources; the worker's playlist is
        // authoritative for what plays next.
        p.stop();
        p.set_volume(replay_gain_to_volume(gain_db));
        p.append(source);
        p.play();
    } else if let Err(e) = open_first_player(source, gain_db).map(|(s, p)| {
        *sink = Some(s);
        *player = Some(p);
    }) {
        telemetry::log(
            "ERROR",
            "audio.player",
            &format!("start output open failed path={} err={e}", path.display()),
        );
        let mut snapshot = shared.lock().expect("player poisoned");
        snapshot.state = TransportState::Stopped;
        snapshot.current = Some(path.to_path_buf());
        return None;
    }
    let mut snapshot = shared.lock().expect("player poisoned");
    snapshot.state = TransportState::Playing;
    snapshot.current = Some(path.to_path_buf());
    snapshot.position_secs = 0.0;
    snapshot.total_secs = total_secs;
    Some(total_secs)
}

/// Publish the worker playlist for `Player::queue_view()` readers.
fn publish_queue(
    worker_queue: &Arc<Mutex<WorkerQueue>>,
    tracks: &[QueueTrack],
    playhead: Option<usize>,
) {
    let mut q = worker_queue.lock().expect("worker queue poisoned");
    q.tracks = tracks.to_vec();
    q.playhead = playhead;
}

/// Worker thread: drain commands, drive rodio, and — on track-end — advance
/// the playhead (wrapping around for a circular playlist). A 25 ms
/// `recv_timeout` tick re-checks the roll-over and keeps the position fresh.
#[allow(clippy::too_many_lines)]
fn worker_loop(
    rx: Receiver<PlayerCommand>,
    shared: Arc<Mutex<PlayerSnapshot>>,
    worker_queue: Arc<Mutex<WorkerQueue>>,
) {
    telemetry::log("INFO", "audio.player", "worker_loop starting");
    let mut rodio_sink: Option<rodio::MixerDeviceSink> = None;
    let mut rodio_player: Option<RodioPlayer> = None;
    let mut tracks: Vec<QueueTrack> = Vec::new();
    let mut playhead: Option<usize> = None;
    // `halted` is true after an explicit `Stop`; suppresses auto-advance so
    // a stopped track (whose source then runs out) doesn't immediately play
    // the next one.
    let mut halted = false;
    let mut cmd_n = 0u64;

    {
        let mut q = worker_queue.lock().expect("worker queue poisoned");
        q.ready = true;
    }

    // Start the track at `i` and report success.
    // (Inlined as a closure-friendly helper since it needs &mut to the
    // rodio state, which a closure here would fight over, so it's a fn.)
    macro_rules! start_at {
        ($i:expr) => {{
            let (p, g) = tracks[$i].clone();
            start_track(&mut rodio_player, &mut rodio_sink, &p, g, &shared).is_some()
        }};
    }

    loop {
        // Track-end roll-over: the current source finished and we have a
        // playlist → advance the playhead (circular). Skipped while halted.
        if !halted
            && !tracks.is_empty()
            && playhead.is_some()
            && rodio_player.as_ref().is_some_and(|p| p.len() == 0)
        {
            let n = tracks.len();
            let cur = playhead.expect("checked");
            let next_i = (cur + 1) % n;
            let (next_path, next_gain) = tracks[next_i].clone();
            telemetry::log(
                "INFO",
                "audio.player",
                &format!("roll over to index={next_i} path={}", next_path.display()),
            );
            if start_track(
                &mut rodio_player,
                &mut rodio_sink,
                &next_path,
                next_gain,
                &shared,
            )
            .is_some()
            {
                playhead = Some(next_i);
            }
            publish_queue(&worker_queue, &tracks, playhead);
            continue;
        }

        match rx.recv_timeout(Duration::from_millis(25)) {
            Ok(cmd) => {
                cmd_n += 1;
                telemetry::log(
                    "DEBUG",
                    "audio.player",
                    &format!("recv cmd #{cmd_n} = {cmd:?}"),
                );
                match cmd {
                    PlayerCommand::Play(path, gain) => {
                        halted = false;
                        tracks.clear();
                        tracks.push((path.clone(), gain));
                        if start_track(&mut rodio_player, &mut rodio_sink, &path, gain, &shared)
                            .is_some()
                        {
                            playhead = Some(0);
                        }
                        telemetry::log(
                            "INFO",
                            "audio.player",
                            &format!("Play start path={}", path.display()),
                        );
                    }
                    PlayerCommand::PlayAndEnqueue { first, gain, rest } => {
                        halted = false;
                        tracks = std::iter::once((first.clone(), gain)).chain(rest).collect();
                        if start_track(&mut rodio_player, &mut rodio_sink, &first, gain, &shared)
                            .is_some()
                        {
                            playhead = Some(0);
                        }
                        telemetry::log(
                            "INFO",
                            "audio.player",
                            &format!(
                                "PlayAndEnqueue path={} rest={}",
                                first.display(),
                                tracks.len()
                            ),
                        );
                    }
                    PlayerCommand::Enqueue(path, gain) => {
                        halted = false;
                        tracks.push((path.clone(), gain));
                        if playhead.is_none() {
                            playhead = Some(tracks.len() - 1);
                        }
                        telemetry::log(
                            "INFO",
                            "audio.player",
                            &format!("enqueue path={} len={}", path.display(), tracks.len()),
                        );
                    }
                    PlayerCommand::Next => {
                        if tracks.is_empty() {
                            continue;
                        }
                        halted = false;
                        let n = tracks.len();
                        let next_i = match playhead {
                            None => 0,
                            Some(c) => (c + 1) % n,
                        };
                        if start_at!(next_i) {
                            playhead = Some(next_i);
                        }
                        telemetry::log("INFO", "audio.player", &format!("Next → index={next_i}"));
                    }
                    PlayerCommand::Previous => {
                        if tracks.is_empty() {
                            continue;
                        }
                        halted = false;
                        let n = tracks.len();
                        let prev_i = match playhead {
                            None => 0,
                            Some(0) => n - 1,
                            Some(c) => c - 1,
                        };
                        if start_at!(prev_i) {
                            playhead = Some(prev_i);
                        }
                        telemetry::log(
                            "INFO",
                            "audio.player",
                            &format!("Previous → index={prev_i}"),
                        );
                    }
                    PlayerCommand::PlayAt(i) => {
                        if i < tracks.len() {
                            halted = false;
                            if start_at!(i) {
                                playhead = Some(i);
                            }
                        } else {
                            telemetry::log(
                                "WARN",
                                "audio.player",
                                &format!("PlayAt out of range i={i}"),
                            );
                        }
                    }
                    PlayerCommand::RemoveAt(i) => {
                        if i < tracks.len() {
                            tracks.remove(i);
                            let n = tracks.len();
                            match playhead {
                                Some(c) if c == i => {
                                    // Removed the current track: start the one
                                    // that slid into slot `i`, or stop if none.
                                    halted = false;
                                    if n > 0 {
                                        let new_i = i.min(n - 1);
                                        if start_at!(new_i) {
                                            playhead = Some(new_i);
                                        }
                                    } else {
                                        playhead = None;
                                        let mut snap = shared.lock().expect("player poisoned");
                                        snap.state = TransportState::Stopped;
                                    }
                                }
                                Some(c) if c > i => playhead = Some(c - 1),
                                _ => {}
                            }
                            telemetry::log(
                                "INFO",
                                "audio.player",
                                &format!("RemoveAt {i} → len={n}"),
                            );
                        }
                    }
                    PlayerCommand::Move { from, to } => {
                        if from != to && from < tracks.len() && to < tracks.len() {
                            let item = tracks.remove(from);
                            tracks.insert(to, item);
                            match playhead {
                                Some(c) if c == from => playhead = Some(to),
                                Some(c) if from < c && to >= c => playhead = Some(c - 1),
                                Some(c) if from > c && to <= c => playhead = Some(c + 1),
                                _ => {}
                            }
                        }
                    }
                    PlayerCommand::ClearQueue => {
                        tracks.clear();
                        playhead = None;
                        halted = true;
                        if let Some(p) = rodio_player.as_ref() {
                            p.stop();
                        }
                        let mut snap = shared.lock().expect("player poisoned");
                        snap.state = TransportState::Stopped;
                        snap.current = None;
                        snap.position_secs = 0.0;
                        snap.total_secs = 0.0;
                        telemetry::log("INFO", "audio.player", "playlist cleared");
                    }
                    PlayerCommand::Pause => {
                        if let Some(p) = rodio_player.as_ref() {
                            p.pause();
                        }
                        let mut snapshot = shared.lock().expect("player poisoned");
                        snapshot.state = snapshot.state.pause();
                        snapshot.position_secs = rodio_player
                            .as_ref()
                            .map(|p| p.get_pos().as_secs_f32())
                            .unwrap_or_default();
                    }
                    PlayerCommand::Resume => {
                        halted = false;
                        if let Some(p) = rodio_player.as_ref() {
                            p.play();
                        }
                        let mut snapshot = shared.lock().expect("player poisoned");
                        snapshot.state = snapshot.state.resume();
                    }
                    PlayerCommand::Stop => {
                        halted = true;
                        if let Some(p) = rodio_player.as_ref() {
                            p.stop();
                        }
                        // New behaviour: Stop is just "halt" — the playlist and
                        // playhead persist (Next resumes from where we were).
                        // `current` (the track playing *right now*) still reads
                        // None since nothing is audible.
                        let mut snapshot = shared.lock().expect("player poisoned");
                        snapshot.state = TransportState::Stopped;
                        snapshot.current = None;
                        snapshot.position_secs = 0.0;
                    }
                }
                publish_queue(&worker_queue, &tracks, playhead);
            }
            Err(RecvTimeoutError::Timeout) => {
                // tick: keep the position snapshot fresh while playing
                if let (Some(p), Some(_)) = (rodio_player.as_ref(), playhead) {
                    let pos = p.get_pos().as_secs_f32();
                    let mut snap = shared.lock().expect("player poisoned");
                    if (snap.position_secs - pos).abs() > 1.0 {
                        snap.position_secs = pos;
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
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
