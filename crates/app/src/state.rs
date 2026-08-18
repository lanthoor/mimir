//! Shared state behind every IPC command.
//!
//! Holds the open `Library`, a `Transport`, and a worker handle that drains
//! `ScanJob`s into the metadata pipeline.
//!
//! On construction the state auto-opens the user's default library location
//! (per the OS data-dir convention). If the open fails — for example, the
//! data dir is not writable — `last_error` is set and the SPA can surface
//! it via the `library_status` IPC. The user can still call
//! `library_open` again with a different path to recover.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

use mimir_audio::{Transport, TransportCommand};
use mimir_core::db::Library;
use mimir_core::scanner::ScanJob;
use serde::Serialize;

use crate::error::AppError;

/// Snapshot of the library status for the front-end.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LibraryStatus {
    /// Path the library was opened at, if any.
    pub path: Option<PathBuf>,
    /// Most recent open error, if any. Cleared on the next successful open.
    pub last_error: Option<String>,
}

/// Shared state handed to every Tauri command via `tauri::State`.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    library: Option<Library>,
    transport: Transport,
    /// Sender end of the scan-worker channel. Drop to shut the worker down.
    scan_tx: Option<Sender<ScanJob>>,
    status: LibraryStatus,
    /// Optional player for actual audio output. Populated when the `output`
    /// feature is enabled.
    #[cfg(feature = "output")]
    player: Option<mimir_audio::Player>,
}

impl AppState {
    /// Construct the state and auto-open the user's default library.
    ///
    /// Construction never panics: if the implicit open fails, the error
    /// is captured in `library_status` and the user can recover by
    /// calling `library_open` with a different path.
    pub fn new() -> Self {
        let state = Self::default();
        let path = default_library_path();
        if let Err(e) = state.open_library(&path) {
            // `open_library` already updates the status on failure; this
            // branch is here for documentation. Don't re-error.
            let _ = e;
        }
        state
    }

    /// Open (or create) the library at `path`. Idempotent.
    ///
    /// On success the previous `last_error` is cleared; on failure the
    /// path is recorded as the attempted path, `last_error` captures the
    /// message, and any previously-open library is closed. This makes the
    /// state consistent: either the library at `status.path` is open, or
    /// it's not.
    pub fn open_library(&self, path: &Path) -> Result<(), AppError> {
        let mut inner = self.inner.lock().expect("state poisoned");
        inner.status.path = Some(path.to_path_buf());
        match Library::open(path) {
            Ok(lib) => {
                inner.library = Some(lib);
                inner.status.last_error = None;
                Ok(())
            }
            Err(e) => {
                let msg = e.to_string();
                inner.library = None;
                inner.status.last_error = Some(msg);
                Err(AppError::from(e))
            }
        }
    }

    /// True when the library is open and queries can be run.
    pub fn is_open(&self) -> bool {
        let inner = self.inner.lock().expect("state poisoned");
        inner.library.is_some()
    }

    /// Snapshot the current library status for the front-end.
    pub fn library_status(&self) -> LibraryStatus {
        let inner = self.inner.lock().expect("state poisoned");
        inner.status.clone()
    }

    pub fn library(&self) -> Result<Library, AppError> {
        let inner = self.inner.lock().expect("state poisoned");
        let lib = inner
            .library
            .clone()
            .ok_or_else(|| AppError::Internal("library not opened yet".into()))?;
        drop(inner);
        Ok(lib)
    }

    /// Enqueue a folder for scanning. The actual scan is async — this
    /// command returns once the folder row is upserted. A scan worker drains
    /// the channel on a background thread.
    pub fn add_folder(&self, root: &Path) -> Result<i64, AppError> {
        let lib = self.library()?;
        let conn = lib.conn()?;
        let folder_id = mimir_core::scanner::upsert_folder(&conn, root)?;

        // Spawn a worker on first call.
        let mut inner = self.inner.lock().expect("state poisoned");
        if inner.scan_tx.is_none() {
            let (tx, rx) = channel::<ScanJob>();
            let worker_lib = lib.clone();
            std::thread::spawn(move || {
                mimir_core::metadata::run_worker(&worker_lib.conn().expect("conn"), rx);
            });
            inner.scan_tx = Some(tx);
        }
        drop(inner);

        // Walk + emit jobs synchronously here; the worker picks them up.
        let tx = self
            .inner
            .lock()
            .expect("state poisoned")
            .scan_tx
            .as_ref()
            .expect("scan_tx")
            .clone();
        let conn = lib.conn()?;
        mimir_core::scanner::scan_root(&conn, root, tx)?;
        Ok(folder_id)
    }

    pub fn search(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<mimir_core::query::TrackRow>, AppError> {
        let lib = self.library()?;
        let conn = lib.conn()?;
        Ok(mimir_core::query::search_tracks(&conn, query, limit)?)
    }

    /// Return the cover art attached to `album_id`, if any. The cover is
    /// returned as `(mime_type, bytes)` so the front-end can render it
    /// directly via a `data:` URL or `Blob`.
    // ponytail: single IPC round-trip per album; covers above ~2 MB will
    // degrade the WebView serialise step. Switch to a Tauri channel and
    // stream bytes if user libraries routinely hold >5 MB scans.
    pub fn album_cover(&self, album_id: i64) -> Result<Option<(String, Vec<u8>)>, AppError> {
        let lib = self.library()?;
        let conn = lib.conn()?;
        let row = mimir_core::db::album_cover(&conn, album_id)?;
        Ok(row.map(|c| (c.mime_type, c.data)))
    }

    pub fn transport(&self) -> Transport {
        self.inner.lock().expect("state poisoned").transport.clone()
    }

    pub fn send_transport(&self, cmd: TransportCommand) {
        let mut inner = self.inner.lock().expect("state poisoned");
        inner.transport.dispatch(cmd);
    }

    /// Look up the track's path in the library and start playback.
    ///
    /// Drives both the (legacy) transport state machine and the real
    /// player — the transport is what the IPC handlers see, the player
    /// is what actually produces sound (when the `output` feature is on).
    pub fn play_track(
        &self,
        track_id: i64,
        transport_cmd: &TransportCommand,
    ) -> Result<(), AppError> {
        // Update the transport state first so the UI sees Playing immediately.
        self.send_transport(transport_cmd.clone());

        // Look up the file path.
        let lib = self.library()?;
        let conn = lib.conn()?;
        let path: Option<PathBuf> = conn
            .query_row("SELECT path FROM track WHERE id = ?1", [track_id], |row| {
                row.get::<_, String>(0)
            })
            .ok()
            .map(PathBuf::from);
        let Some(path) = path else {
            return Err(AppError::Internal(format!("no track with id {track_id}")));
        };

        #[cfg(feature = "output")]
        {
            use mimir_audio::PlayerCommand;
            let mut inner = self.inner.lock().expect("state poisoned");
            if inner.player.is_none() {
                inner.player = Some(mimir_audio::Player::new());
            }
            let player = inner.player.as_ref().expect("just initialized");
            player
                .handle()
                .send(PlayerCommand::Play(path))
                .map_err(AppError::from)?;
        }

        // When the `output` feature is off, the transport state is the only
        // signal we have. The legacy `transport` is still useful for the UI.
        #[cfg(not(feature = "output"))]
        {
            let _ = path;
        }

        Ok(())
    }

    /// Snapshot of the player for the front-end — None when the `output`
    /// feature is disabled.
    #[cfg(feature = "output")]
    pub fn player_snapshot(&self) -> Option<mimir_audio::PlayerSnapshot> {
        let inner = self.inner.lock().expect("state poisoned");
        inner.player.as_ref().map(mimir_audio::Player::snapshot)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                library: None,
                transport: Transport::default(),
                scan_tx: None,
                status: LibraryStatus::default(),
                #[cfg(feature = "output")]
                player: None,
            })),
        }
    }
}

/// Resolve the default library path: `<data_dir>/mimir/library.sqlite`.
///
/// `dirs::data_dir()` returns the OS-specific per-user data directory
/// (`~/.local/share` on Linux, `~/Library/Application Support` on macOS,
/// `%APPDATA%` on Windows). The parent directory is created if missing.
fn default_library_path() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("mimir");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("library.sqlite")
}
