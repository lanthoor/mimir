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
#[cfg(feature = "tauri")]
use mimir_core::rusqlite;
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

    /// Add multiple folders in one go. Each is upserted by path so re-adding
    /// the same root is a no-op. Workers drain scan jobs on a shared thread.
    pub fn add_folders<I, P>(&self, paths: I) -> Result<Vec<i64>, AppError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let mut ids = Vec::new();
        for p in paths {
            ids.push(self.add_folder(p.as_ref())?);
        }
        Ok(ids)
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

    /// Paged list of albums.
    pub fn list_albums(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<mimir_core::query::AlbumRow>, AppError> {
        let lib = self.library()?;
        let conn = lib.conn()?;
        Ok(mimir_core::query::list_albums(&conn, limit, offset)?)
    }

    /// Distinct genres in the library.
    pub fn list_genres(&self) -> Result<Vec<mimir_core::query::GenreRow>, AppError> {
        let lib = self.library()?;
        let conn = lib.conn()?;
        Ok(mimir_core::query::list_genres(&conn)?)
    }

    /// Distinct years (from albums) in the library.
    pub fn list_years(&self) -> Result<Vec<mimir_core::query::YearRow>, AppError> {
        let lib = self.library()?;
        let conn = lib.conn()?;
        Ok(mimir_core::query::list_years(&conn)?)
    }

    /// Tracks filtered by an optional combination of facets.
    #[allow(clippy::too_many_arguments)]
    pub fn query_tracks_filtered(
        &self,
        genre: Option<String>,
        year: Option<i32>,
        artist_id: Option<i64>,
        album_id: Option<i64>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<mimir_core::query::TrackRow>, AppError> {
        let lib = self.library()?;
        let conn = lib.conn()?;
        let filter = mimir_core::query::TrackFilter {
            genre,
            year,
            artist_id,
            album_id,
        };
        Ok(mimir_core::query::list_tracks_filtered(
            &conn, &filter, limit, offset,
        )?)
    }

    /// Fetch the editable subset of a track.
    #[cfg(feature = "tauri")]
    pub fn get_editable_track(
        &self,
        track_id: i64,
    ) -> Result<crate::command::EditableTrackFields, AppError> {
        use crate::command::EditableTrackFields;
        let lib = self.library()?;
        let conn = lib.conn()?;
        let row: (
            Option<String>,
            Option<String>,
            Option<i32>,
            Option<i32>,
            Option<i32>,
        ) = conn
            .query_row(
                "SELECT t.title, t.genre, a.year, t.track_no, t.disc_no \
                 FROM track t LEFT JOIN album a ON a.id = t.album_id \
                 WHERE t.id = ?1",
                [track_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::Internal(format!("no track with id {track_id}"))
                }
                other => AppError::Sqlite(other.to_string()),
            })?;
        Ok(EditableTrackFields {
            title: row.0,
            genre: row.1,
            year: row.2,
            track_no: row.3,
            disc_no: row.4,
        })
    }

    /// Apply an edit patch to a track. DB-only; never writes the file.
    #[cfg(feature = "tauri")]
    pub fn update_track(
        &self,
        track_id: i64,
        patch: mimir_core::db::TrackPatch,
    ) -> Result<(), AppError> {
        let lib = self.library()?;
        let conn = lib.conn()?;
        mimir_core::db::update_track(&conn, track_id, &patch)?;
        Ok(())
    }

    /// Lyrics for a track, if any.
    #[cfg(feature = "tauri")]
    pub fn track_lyrics(
        &self,
        track_id: i64,
    ) -> Result<Option<mimir_core::db::LyricsRow>, AppError> {
        let lib = self.library()?;
        let conn = lib.conn()?;
        Ok(mimir_core::db::track_lyrics(&conn, track_id)?)
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

        // Look up the file path + ReplayGain gain.
        let lib = self.library()?;
        let conn = lib.conn()?;
        let row: Option<(String, Option<f64>, Option<f64>)> = conn
            .query_row(
                "SELECT path, replaygain_track_db, replaygain_album_db \
                 FROM track WHERE id = ?1",
                [track_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();
        let Some((path_str, track_db, album_db)) = row else {
            return Err(AppError::Internal(format!("no track with id {track_id}")));
        };
        let path = PathBuf::from(path_str);

        #[cfg(feature = "output")]
        {
            use mimir_audio::PlayerCommand;
            let mut inner = self.inner.lock().expect("state poisoned");
            if inner.player.is_none() {
                inner.player = Some(mimir_audio::Player::new());
            }
            let player = inner.player.as_ref().expect("just initialized");
            // Prefer album gain over track gain; pass None when neither exists.
            let gain = album_db.or(track_db);
            player
                .handle()
                .send(PlayerCommand::SetReplayGainDb(gain))
                .map_err(AppError::from)?;
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
            let _ = track_db;
            let _ = album_db;
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
