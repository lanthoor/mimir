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

#[cfg(feature = "output")]
use mimir_audio::{PlayerCommand, Transport, TransportCommand};
use mimir_core::db::Library;
use mimir_core::rusqlite;
use mimir_core::scanner::ScanJob;
use mimir_telemetry as telemetry;
use serde::Serialize;

use crate::error::AppError;

#[cfg(feature = "tauri")]
use crate::command::EditableTrackFields;

/// Tuple shape of `SELECT t.title, t.genre, a.year, t.track_no, t.disc_no FROM track ...`.
/// Used by `get_editable_track` to avoid a wide anonymous struct.
#[cfg(feature = "tauri")]
type EditableTrackRow = (
    Option<String>,
    Option<String>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

/// Snapshot of the library status for the front-end.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LibraryStatus {
    /// Path the library was opened at, if any.
    pub path: Option<PathBuf>,
    /// Most recent open error, if any. Cleared on the next successful open.
    pub last_error: Option<String>,
}

impl Inner {
    /// Lazily create the audio player (shared across the session).
    #[cfg(feature = "output")]
    fn ensure_player(&mut self) -> Result<&mimir_audio::Player, AppError> {
        if self.player.is_none() {
            telemetry::log("INFO", "app", "instantiating audio Player");
            self.player = Some(mimir_audio::Player::new());
        }
        self.player
            .as_ref()
            .ok_or_else(|| AppError::Internal("player not available".into()))
    }
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
        // Best-effort init; missing home is logged via stderr already.
        let _log_guard = telemetry::init();
        telemetry::log("INFO", "app", "mimir starting up");
        telemetry::log(
            "INFO",
            "app",
            &format!("version={} toolchain=stable", env!("CARGO_PKG_VERSION")),
        );

        let state = Self::default();
        let path = default_library_path();
        telemetry::log(
            "INFO",
            "app",
            &format!("implicit open target path={}", path.display()),
        );
        if let Err(e) = state.open_library(&path) {
            telemetry::log("WARN", "app", &format!("implicit library open failed: {e}"));
        }
        telemetry::log(
            "INFO",
            "app",
            &format!(
                "AppState ready is_open={} path={:?}",
                state.is_open(),
                state.library_status().path
            ),
        );
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
        telemetry::log(
            "INFO",
            "app",
            &format!("open_library requested path={}", path.display()),
        );
        let mut inner = self.inner.lock().expect("state poisoned");
        inner.status.path = Some(path.to_path_buf());
        match Library::open(path) {
            Ok(lib) => {
                inner.library = Some(lib);
                inner.status.last_error = None;
                telemetry::log(
                    "INFO",
                    "app",
                    &format!("open_library ok path={}", path.display()),
                );
                Ok(())
            }
            Err(e) => {
                let msg = e.to_string();
                inner.library = None;
                inner.status.last_error = Some(msg.clone());
                telemetry::log(
                    "ERROR",
                    "app",
                    &format!("open_library failed path={} err={msg}", path.display()),
                );
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
        let lib = inner.library.clone().ok_or_else(|| {
            telemetry::log("WARN", "app", "library() called without an open library");
            AppError::Internal("library not opened yet".into())
        })?;
        drop(inner);
        Ok(lib)
    }

    /// Upsert the folder row, kick off the scan on a background thread, and
    /// return the `folder_id` immediately. The actual walk + hash runs off
    /// the main thread so the IPC handler (and the UI) never block on it.
    ///
    /// `on_done` is invoked from the scan thread with the `ScanSummary` on
    /// success or the error string on failure. Pass `None` in tests / callers
    /// that don't want Tauri events.
    pub fn add_folder<F>(&self, root: &Path, on_done: Option<F>) -> Result<i64, AppError>
    where
        F: FnOnce(Result<mimir_core::scanner::ScanSummary, String>) + Send + 'static,
    {
        telemetry::log(
            "INFO",
            "app",
            &format!("add_folder start root={}", root.display()),
        );
        let lib = self.library()?;
        let conn = lib.conn()?;
        let folder_id = match mimir_core::scanner::upsert_folder(&conn, root) {
            Ok(id) => {
                telemetry::log(
                    "DEBUG",
                    "app",
                    &format!("add_folder: upsert_folder ok id={id}"),
                );
                id
            }
            Err(e) => {
                telemetry::log(
                    "ERROR",
                    "app",
                    &format!(
                        "add_folder: upsert_folder failed root={} err={e}",
                        root.display()
                    ),
                );
                return Err(e.into());
            }
        };

        let tx = self.ensure_scan_worker(&lib, root);
        spawn_scan_thread(lib, root.to_path_buf(), tx, on_done);

        Ok(folder_id)
    }

    /// Make sure the scan-worker thread is running and return a clone of its
    /// `Sender`. Spawns the worker on first call.
    fn ensure_scan_worker(&self, lib: &mimir_core::db::Library, root: &Path) -> Sender<ScanJob> {
        let mut inner = self.inner.lock().expect("state poisoned");
        if let Some(tx) = inner.scan_tx.as_ref() {
            return tx.clone();
        }
        let (tx, rx) = channel::<ScanJob>();
        let worker_lib = lib.clone();
        let target = root.to_path_buf();
        std::thread::spawn(move || {
            telemetry::log(
                "INFO",
                "app",
                &format!("scan worker thread spawned target={}", target.display()),
            );
            mimir_core::metadata::run_worker(&worker_lib, rx);
        });
        inner.scan_tx = Some(tx.clone());
        telemetry::log("INFO", "app", "scan_tx initialised");
        tx
    }

    /// Add multiple folders. Each upsert is fast; the scan runs in the
    /// background per folder. Returns the list of `folder_id`s on success.
    /// Callers that need per-folder completion callbacks should iterate
    /// `add_folder` themselves.
    pub fn add_folders<I, P>(&self, paths: I) -> Result<Vec<i64>, AppError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        telemetry::log("INFO", "app", "add_folders enter batch");
        let mut out: Vec<i64> = Vec::new();
        let mut n = 0u64;
        for p in paths {
            n += 1;
            match self.add_folder::<fn(Result<mimir_core::scanner::ScanSummary, String>)>(
                p.as_ref(),
                None,
            ) {
                Ok(id) => out.push(id),
                Err(e) => {
                    telemetry::log(
                        "ERROR",
                        "app",
                        &format!(
                            "add_folders child #{n} failed path={} err={e}",
                            p.as_ref().display()
                        ),
                    );
                    return Err(e);
                }
            }
        }
        telemetry::log(
            "INFO",
            "app",
            &format!("add_folders done ok={} attempted={n}", out.len()),
        );
        Ok(out)
    }

    pub fn search(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<mimir_core::query::TrackRow>, AppError> {
        telemetry::log(
            "INFO",
            "app",
            &format!("search query={query:?} limit={limit}"),
        );
        let lib = self.library()?;
        let conn = lib.conn()?;
        let out = mimir_core::query::search_tracks(&conn, query, limit)?;
        telemetry::log("INFO", "app", &format!("search ok n={}", out.len()));
        Ok(out)
    }

    /// Folders-view backing list (one row per watched root).
    #[cfg(feature = "tauri")]
    pub fn list_folders(&self) -> Result<Vec<crate::command::FolderRow>, AppError> {
        telemetry::log("DEBUG", "app", "list_folders request");
        let lib = self.library()?;
        let conn = lib.conn()?;
        let out = mimir_core::query::list_folders(&conn)?;
        let rows: Vec<crate::command::FolderRow> = out
            .root_children
            .into_iter()
            .filter_map(|n| {
                let id = n.folder_id?;
                Some(crate::command::FolderRow {
                    file_count: count_files(&n),
                    path: n.path,
                    id,
                })
            })
            .collect();
        telemetry::log("INFO", "app", &format!("list_folders ok n={}", rows.len()));
        Ok(rows)
    }

    /// Full folder tree for the Folders view (icon + tree shapes).
    #[cfg(feature = "tauri")]
    pub fn folder_tree(&self) -> Result<mimir_core::query::FolderView, AppError> {
        telemetry::log("DEBUG", "app", "folder_tree request");
        let lib = self.library()?;
        let conn = lib.conn()?;
        let out = mimir_core::query::list_folders(&conn)?;
        telemetry::log(
            "INFO",
            "app",
            &format!(
                "folder_tree ok flat={} roots={}",
                out.flat.len(),
                out.root_children.len()
            ),
        );
        Ok(out)
    }

    /// Mark a watched folder inactive. Returns an `Internal` error if
    /// the id is unknown; otherwise this is silent because the Folders
    /// view re-fetches.
    #[cfg(feature = "tauri")]
    pub fn remove_folder(&self, folder_id: i64) -> Result<(), AppError> {
        telemetry::log(
            "INFO",
            "app",
            &format!("remove_folder folder_id={folder_id}"),
        );
        let lib = self.library()?;
        let conn = lib.conn()?;
        let changed = conn.execute(
            "UPDATE folder SET active = 0 WHERE id = ?1 AND active = 1",
            [folder_id],
        )?;
        if changed == 0 {
            telemetry::log(
                "WARN",
                "app",
                &format!("remove_folder: no active row folder_id={folder_id}"),
            );
            return Err(AppError::Internal(format!(
                "folder {folder_id} not found or already removed"
            )));
        }
        telemetry::log(
            "INFO",
            "app",
            &format!("remove_folder ok folder_id={folder_id}"),
        );
        Ok(())
    }

    /// Rename a watched folder's on-disk path. Updates `folder.path` and
    /// rewrites every `track.path` that pointed at the old prefix so the
    /// Folders view + playback track the live FS location. Tracks whose
    /// path doesn't start with the old prefix are left alone (the user
    /// may have already moved them to another root).
    #[cfg(feature = "tauri")]
    pub fn rename_folder(&self, folder_id: i64, new_path: &str) -> Result<(), AppError> {
        telemetry::log(
            "INFO",
            "app",
            &format!("rename_folder folder_id={folder_id} new_path={new_path}"),
        );
        let lib = self.library()?;
        let conn = lib.conn()?;

        // Look up the existing path; need it to rewrite matching tracks.
        let old_path: String = conn
            .query_row(
                "SELECT path FROM folder WHERE id = ?1",
                [folder_id],
                |row| row.get(0),
            )
            .map_err(|_| AppError::Internal(format!("folder {folder_id} not found")))?;

        if old_path == new_path {
            telemetry::log("DEBUG", "app", "rename_folder: no-op (same path)");
            return Ok(());
        }

        // Update the folder row + any tracks underneath it.
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE folder SET path = ?1 WHERE id = ?2",
            rusqlite::params![new_path, folder_id],
        )?;
        let suffix = format!("{old_path}/");
        // Rewrite track paths whose stored string still has the old
        // prefix. `path LIKE 'old/%'` covers the recursive subdir case;
        // the `substr(1 + len)` swaps the prefix in place.
        let rewritten = tx.execute(
            "UPDATE track SET path = ?1 || substr(path, ?2) \
             WHERE path LIKE ?3 ESCAPE '\\'",
            rusqlite::params![
                new_path,
                i64::try_from(suffix.len()).expect("path fits"),
                format!("{}{}", escape_like_folder(&suffix), "%"),
            ],
        )?;
        tx.commit()?;
        telemetry::log(
            "INFO",
            "app",
            &format!("rename_folder ok folder_id={folder_id} tracks_rewritten={rewritten}"),
        );
        Ok(())
    }

    /// Rename a subdirectory under a watched root. The new name is a
    /// single path segment (no separators) — the backend derives the
    /// full new path from the parent of the current path + the new name.
    /// Actually renames the directory on disk and rewrites every
    /// `track.path` underneath it.
    #[cfg(feature = "tauri")]
    pub fn rename_subdir(&self, current_path: &str, new_name: &str) -> Result<(), AppError> {
        telemetry::log(
            "INFO",
            "app",
            &format!("rename_subdir current={current_path} new_name={new_name}"),
        );

        // Validate the new name: a single segment, no separators.
        if new_name.is_empty()
            || new_name.contains('/')
            || new_name.contains('\\')
            || new_name.contains('\0')
        {
            return Err(AppError::Internal(
                "new name must be a single path segment".into(),
            ));
        }

        let current = std::path::Path::new(current_path);
        let parent = current
            .parent()
            .ok_or_else(|| AppError::Internal("cannot rename a root path".into()))?;
        let new_path = parent.join(new_name);

        if new_path.exists() {
            return Err(AppError::Internal(format!(
                "target already exists: {}",
                new_path.display()
            )));
        }

        // Rename on disk.
        std::fs::rename(current, &new_path).map_err(|e| {
            telemetry::log(
                "ERROR",
                "app",
                &format!("rename_subdir fs rename failed: {e}"),
            );
            AppError::Io(e.to_string())
        })?;

        // Rewrite track paths in the DB.
        let lib = self.library()?;
        let conn = lib.conn()?;
        let suffix = format!("{current_path}/");
        let new_path_str = new_path.to_string_lossy().into_owned();
        let rewritten = conn.execute(
            "UPDATE track SET path = ?1 || substr(path, ?2) \
             WHERE path LIKE ?3 ESCAPE '\\'",
            rusqlite::params![
                &new_path_str,
                i64::try_from(suffix.len()).expect("path fits"),
                format!("{}{}", escape_like_folder(&suffix), "%"),
            ],
        )?;
        telemetry::log(
            "INFO",
            "app",
            &format!("rename_subdir ok new_path={new_path_str} tracks_rewritten={rewritten}"),
        );
        Ok(())
    }

    /// Reveal a file (or directory) in the platform's file manager.
    /// Linux: `xdg-open <parent_dir>`. macOS: `open -R <path>`.
    /// Windows: `explorer /select,<path>`.
    #[cfg(feature = "tauri")]
    pub fn reveal_in_file_manager(&self, path: &str) -> Result<(), AppError> {
        telemetry::log(
            "INFO",
            "app",
            &format!("reveal_in_file_manager path={path}"),
        );
        let p = std::path::Path::new(path);
        #[cfg(target_os = "linux")]
        {
            let dir = if p.is_dir() {
                p.to_path_buf()
            } else {
                p.parent()
                    .map(std::path::Path::to_path_buf)
                    .unwrap_or_default()
            };
            std::process::Command::new("xdg-open")
                .arg(&dir)
                .status()
                .map_err(|e| AppError::Io(e.to_string()))?;
        }
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg("-R")
                .arg(p)
                .status()
                .map_err(|e| AppError::Io(e.to_string()))?;
        }
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(format!("/select,{}", p.display()))
                .status()
                .map_err(|e| AppError::Io(e.to_string()))?;
        }
        Ok(())
    }

    /// Return the cover art attached to `album_id`, if any. The cover is
    /// returned as `(mime_type, bytes)` so the front-end can render it
    /// directly via a `data:` URL or `Blob`.
    // ponytail: single IPC round-trip per album; covers above ~2 MB will
    // degrade the WebView serialise step. Switch to a Tauri channel and
    // stream bytes if user libraries routinely hold >5 MB scans.
    pub fn album_cover(&self, album_id: i64) -> Result<Option<(String, Vec<u8>)>, AppError> {
        telemetry::log(
            "DEBUG",
            "app",
            &format!("album_cover request album_id={album_id}"),
        );
        let lib = self.library()?;
        let conn = lib.conn()?;
        let row = mimir_core::db::album_cover(&conn, album_id)?;
        telemetry::log(
            "INFO",
            "app",
            &format!(
                "album_cover ok album_id={album_id} present={}",
                row.is_some()
            ),
        );
        Ok(row.map(|c| (c.mime_type, c.data)))
    }

    /// Paged list of albums.
    pub fn list_albums(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<mimir_core::query::AlbumRow>, AppError> {
        telemetry::log(
            "DEBUG",
            "app",
            &format!("list_albums limit={limit} offset={offset}"),
        );
        let lib = self.library()?;
        let conn = lib.conn()?;
        let out = mimir_core::query::list_albums(&conn, limit, offset)?;
        telemetry::log("INFO", "app", &format!("list_albums ok n={}", out.len()));
        Ok(out)
    }

    /// Distinct genres in the library.
    pub fn list_genres(&self) -> Result<Vec<mimir_core::query::GenreRow>, AppError> {
        telemetry::log("DEBUG", "app", "list_genres request");
        let lib = self.library()?;
        let conn = lib.conn()?;
        let out = mimir_core::query::list_genres(&conn)?;
        telemetry::log("INFO", "app", &format!("list_genres ok n={}", out.len()));
        Ok(out)
    }

    /// Distinct years (from albums) in the library.
    pub fn list_years(&self) -> Result<Vec<mimir_core::query::YearRow>, AppError> {
        telemetry::log("DEBUG", "app", "list_years request");
        let lib = self.library()?;
        let conn = lib.conn()?;
        let out = mimir_core::query::list_years(&conn)?;
        telemetry::log("INFO", "app", &format!("list_years ok n={}", out.len()));
        Ok(out)
    }

    /// Tracks filtered by an optional combination of facets.
    #[allow(clippy::too_many_arguments)]
    pub fn list_tracks(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<mimir_core::query::TrackRow>, AppError> {
        telemetry::log(
            "INFO",
            "app",
            &format!("list_tracks limit={limit} offset={offset}"),
        );
        let lib = self.library()?;
        let conn = lib.conn()?;
        let out = mimir_core::query::list_tracks(&conn, limit, offset)?;
        telemetry::log("INFO", "app", &format!("list_tracks ok n={}", out.len()));
        Ok(out)
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
        telemetry::log(
            "INFO",
            "app",
            &format!(
                "query_tracks_filtered genre={genre:?} year={year:?} artist_id={artist_id:?} album_id={album_id:?} limit={limit} offset={offset}"
            ),
        );
        let lib = self.library()?;
        let conn = lib.conn()?;
        let filter = mimir_core::query::TrackFilter {
            genre,
            year,
            artist_id,
            album_id,
        };
        let out = mimir_core::query::list_tracks_filtered(&conn, &filter, limit, offset)?;
        telemetry::log(
            "INFO",
            "app",
            &format!("query_tracks_filtered ok n={}", out.len()),
        );
        Ok(out)
    }

    /// Fetch the editable subset of a track.
    #[cfg(feature = "tauri")]
    pub fn get_editable_track(
        &self,
        track_id: i64,
    ) -> Result<crate::command::EditableTrackFields, AppError> {
        telemetry::log(
            "DEBUG",
            "app",
            &format!("get_editable_track track_id={track_id}"),
        );
        let lib = self.library()?;
        let conn = lib.conn()?;
        let row: EditableTrackRow = conn
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
                    telemetry::log(
                        "WARN",
                        "app",
                        &format!("get_editable_track no row for track_id={track_id}"),
                    );
                    AppError::Internal(format!("no track with id {track_id}"))
                }
                other => {
                    telemetry::log(
                        "ERROR",
                        "app",
                        &format!("get_editable_track sqlite err track_id={track_id} err={other}"),
                    );
                    AppError::Sqlite(other.to_string())
                }
            })?;
        telemetry::log(
            "INFO",
            "app",
            &format!(
                "get_editable_track ok track_id={track_id} title={:?} genre={:?} year={:?} tno={:?} dno={:?}",
                row.0, row.1, row.2, row.3, row.4
            ),
        );
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
        telemetry::log(
            "INFO",
            "app",
            &format!("update_track track_id={track_id} patch={patch:?}"),
        );
        let lib = self.library()?;
        let conn = lib.conn()?;
        mimir_core::db::update_track(&conn, track_id, &patch)?;
        telemetry::log(
            "INFO",
            "app",
            &format!("update_track ok track_id={track_id}"),
        );
        Ok(())
    }

    /// Lyrics for a track, if any.
    #[cfg(feature = "tauri")]
    pub fn track_lyrics(
        &self,
        track_id: i64,
    ) -> Result<Option<mimir_core::db::LyricsRow>, AppError> {
        telemetry::log("DEBUG", "app", &format!("track_lyrics track_id={track_id}"));
        let lib = self.library()?;
        let conn = lib.conn()?;
        let out = mimir_core::db::track_lyrics(&conn, track_id)?;
        telemetry::log(
            "INFO",
            "app",
            &format!(
                "track_lyrics ok track_id={track_id} present={} bytes={}",
                out.is_some(),
                out.as_ref().map_or(0, |r| r.text.len())
            ),
        );
        Ok(out)
    }

    pub fn transport(&self) -> Transport {
        self.inner.lock().expect("state poisoned").transport.clone()
    }

    pub fn send_transport(&self, cmd: TransportCommand) {
        telemetry::log("DEBUG", "app", &format!("send_transport {cmd:?}"));
        let mut inner = self.inner.lock().expect("state poisoned");
        inner.transport.dispatch(cmd);
    }

    /// Send a command to the live audio `Player` worker. No-op when the
    /// `output` feature is off or when no player has been instantiated
    /// yet (the player is lazily created on the first `play_track`).
    #[cfg(feature = "output")]
    pub fn send_player(&self, cmd: mimir_audio::PlayerCommand) {
        telemetry::log("DEBUG", "app", &format!("send_player {cmd:?}"));
        let inner = self.inner.lock().expect("state poisoned");
        if let Some(p) = inner.player.as_ref() {
            if let Err(e) = p.handle().send(cmd) {
                telemetry::log("WARN", "app", &format!("send_player: worker gone: {e}"));
            }
        }
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
        telemetry::log(
            "INFO",
            "app",
            &format!("play_track track_id={track_id} transport_cmd={transport_cmd:?}"),
        );
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
            telemetry::log(
                "WARN",
                "app",
                &format!("play_track: no row track_id={track_id}"),
            );
            return Err(AppError::Internal(format!("no track with id {track_id}")));
        };
        let path = PathBuf::from(path_str);
        telemetry::log(
            "DEBUG",
            "app",
            &format!(
                "play_track path={} rg_track={track_db:?} rg_album={album_db:?}",
                path.display()
            ),
        );

        #[cfg(feature = "output")]
        {
            use mimir_audio::PlayerCommand;
            let mut inner = self.inner.lock().expect("state poisoned");
            if inner.player.is_none() {
                telemetry::log("INFO", "app", "instantiating audio Player");
                inner.player = Some(mimir_audio::Player::new());
            }
            let player = inner.player.as_ref().expect("just initialized");
            // Prefer album gain over track gain; the worker bakes it into
            // the source at start (`Play` carries the gain per track).
            let gain = album_db.or(track_db);
            player
                .handle()
                .send(PlayerCommand::Play(path.clone(), gain))
                .map_err(|e| {
                    telemetry::log(
                        "ERROR",
                        "app",
                        &format!("play_track: Play send err path={}: {e}", path.display()),
                    );
                    AppError::from(e)
                })?;
        }

        // When the `output` feature is off, the transport state is the only
        // signal we have. The legacy `transport` is still useful for the UI.
        #[cfg(not(feature = "output"))]
        {
            telemetry::log(
                "DEBUG",
                "app",
                "play_track: output feature off — transport-only update",
            );
            let _ = path;
            let _ = track_db;
            let _ = album_db;
        }

        telemetry::log("INFO", "app", &format!("play_track ok track_id={track_id}"));
        Ok(())
    }

    /// Snapshot of the player for the front-end — None when the `output`
    /// feature is disabled.
    #[cfg(feature = "output")]
    pub fn player_snapshot(&self) -> Option<mimir_audio::PlayerSnapshot> {
        telemetry::log("DEBUG", "app", "player_snapshot");
        let inner = self.inner.lock().expect("state poisoned");
        inner.player.as_ref().map(mimir_audio::Player::snapshot)
    }

    /// Get a clone of the player handle, lazily creating the player if
    /// needed.
    #[cfg(feature = "output")]
    fn player_or_init(&self) -> Result<mimir_audio::Player, AppError> {
        let mut inner = self.inner.lock().expect("state poisoned");
        Ok(inner.ensure_player()?.clone())
    }

    /// The live player if one has been created, else `None` (does NOT
    /// construct one — used by read-only projections like the queue list).
    /// Returns `None` when the `output` feature is disabled.
    #[cfg(feature = "output")]
    pub fn queue_player(&self) -> Option<mimir_audio::Player> {
        let inner = self.inner.lock().expect("state poisoned");
        inner.player.clone()
    }

    /// Total number of tracks in the playlist, 0 when no player exists yet.
    #[cfg(feature = "output")]
    pub fn queue_len(&self) -> usize {
        self.queue_player()
            .map_or(0, |p| p.queue_view().tracks.len())
    }

    /// Advance to the next track (wraps to the start at the end).
    #[cfg(feature = "output")]
    pub fn queue_next(&self) -> Result<(), AppError> {
        self.player_or_init()?
            .handle()
            .send(mimir_audio::PlayerCommand::Next)
            .map_err(AppError::from)
    }

    /// Step back to the previous track (wraps to the last at the start).
    #[cfg(feature = "output")]
    pub fn queue_previous(&self) -> Result<(), AppError> {
        self.player_or_init()?
            .handle()
            .send(mimir_audio::PlayerCommand::Previous)
            .map_err(AppError::from)
    }

    /// Start the track at playlist position `index` (0-based, the list is a
    /// flat persistent list — removing one just reindexes, no special pin).
    #[cfg(feature = "output")]
    pub fn queue_play_at(&self, index: usize) -> Result<(), AppError> {
        let player = self.player_or_init()?;
        if index >= player.queue_view().tracks.len() {
            return Err(AppError::Internal("play_at index out of range".into()));
        }
        player
            .handle()
            .send(mimir_audio::PlayerCommand::PlayAt(index))
            .map_err(AppError::from)
    }

    /// Append `track_ids` to the END of the queue. When nothing is
    /// playing yet, the first one starts; otherwise they wait in line.
    /// Unresolvable ids are skipped (logged), never fatal. Returns the
    /// ids actually appended, in order.
    #[cfg(feature = "output")]
    pub fn queue_tracks(&self, track_ids: Vec<i64>) -> Result<Vec<i64>, AppError> {
        if track_ids.is_empty() {
            return Ok(Vec::new());
        }
        let lib = self.library()?;
        let resolved: Vec<(i64, PathBuf, Option<f64>)> = Self::resolve_track_ids(&lib, &track_ids);
        // Preserve the caller's order (SQL `IN` returns arbitrary order).
        let ordered = track_ids
            .iter()
            .filter_map(|id| resolved.iter().find(|(rid, _, _)| rid == id))
            .cloned()
            .collect::<Vec<_>>();

        let player = self.player_or_init()?;
        // "Add to queue" always appends; it should only START when the
        // list is empty. A stopped-but-non-empty list is preserved (the
        // playhead is still somewhere in it).
        let list_was_empty = player.queue_view().tracks.is_empty();
        let h = player.handle();
        let mut appended: Vec<i64> = Vec::new();
        for (id, path, gain) in ordered {
            let cmd = if appended.is_empty() && list_was_empty {
                PlayerCommand::Play(path.clone(), gain)
            } else {
                PlayerCommand::Enqueue(path, gain)
            };
            if let Err(e) = h.send(cmd) {
                telemetry::log("ERROR", "app", &format!("queue: send failed id={id}: {e}"));
                continue;
            }
            appended.push(id);
        }
        telemetry::log(
            "INFO",
            "app",
            &format!(
                "queue_tracks requested={} appended={}",
                track_ids.len(),
                appended.len()
            ),
        );
        Ok(appended)
    }

    /// Play `track_ids` in order: the first starts immediately, the rest
    /// are queued behind it — as one atomic command.
    /// Returns the ids actually used, in order.
    #[cfg(feature = "output")]
    pub fn play_and_enqueue(&self, track_ids: Vec<i64>) -> Result<Vec<i64>, AppError> {
        if track_ids.is_empty() {
            return Ok(Vec::new());
        }
        let lib = self.library()?;
        let resolved: Vec<(i64, PathBuf, Option<f64>)> = Self::resolve_track_ids(&lib, &track_ids);
        let ordered = track_ids
            .iter()
            .filter_map(|id| resolved.iter().find(|(rid, _, _)| rid == id))
            .cloned()
            .collect::<Vec<_>>();
        if ordered.is_empty() {
            return Ok(Vec::new());
        }
        let (head, tail) = ordered.split_first().expect("non-empty");
        let rest: Vec<mimir_audio::QueueTrack> =
            tail.iter().map(|(_, p, g)| (p.clone(), *g)).collect();
        let player = self.player_or_init()?;
        let h = player.handle();
        h.send(mimir_audio::PlayerCommand::PlayAndEnqueue {
            first: head.1.clone(),
            gain: head.2,
            rest,
        })
        .map_err(AppError::from)?;
        telemetry::log(
            "INFO",
            "app",
            &format!(
                "play_and_enqueue first={} rest={}",
                head.1.display(),
                tail.len()
            ),
        );
        Ok(ordered.into_iter().map(|(id, _, _)| id).collect())
    }

    /// Remove the track at playlist position `index` (0-based, flat list).
    /// Removing the currently playing track starts the one that slides into
    /// its slot.
    #[cfg(feature = "output")]
    pub fn queue_remove_at(&self, index: usize) -> Result<(), AppError> {
        let player = self.player_or_init()?;
        if index >= player.queue_view().tracks.len() {
            return Err(AppError::Internal("remove_at index out of range".into()));
        }
        player
            .handle()
            .send(mimir_audio::PlayerCommand::RemoveAt(index))
            .map_err(AppError::from)
    }

    /// Move a playlist entry from `from` to `to` (0-based, flat list).
    #[cfg(feature = "output")]
    pub fn queue_move(&self, from: usize, to: usize) -> Result<(), AppError> {
        if from == to {
            return Ok(());
        }
        let player = self.player_or_init()?;
        let len = player.queue_view().tracks.len();
        if from >= len || to >= len {
            return Err(AppError::Internal("move index out of range".into()));
        }
        player
            .handle()
            .send(mimir_audio::PlayerCommand::Move { from, to })
            .map_err(AppError::from)
    }

    /// Clear the queue and stop.
    #[cfg(feature = "output")]
    pub fn queue_clear(&self) -> Result<(), AppError> {
        let player = self.player_or_init()?;
        player
            .handle()
            .send(PlayerCommand::ClearQueue)
            .map_err(AppError::from)
    }

    /// Resolve a list of track ids to `(id, path, replay_gain_db)` tuples.
    /// Order is preserved; missing ids are dropped.
    #[cfg(feature = "output")]
    fn resolve_track_ids(lib: &Library, ids: &[i64]) -> Vec<(i64, PathBuf, Option<f64>)> {
        if ids.is_empty() {
            return Vec::new();
        }
        let conn = match lib.conn() {
            Ok(c) => c,
            Err(e) => {
                telemetry::log("ERROR", "app", &format!("resolve_track_ids conn: {e}"));
                return Vec::new();
            }
        };
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT id, path, COALESCE(replaygain_album_db, replaygain_track_db) \
             FROM track WHERE id IN ({placeholders})"
        );
        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(e) => {
                telemetry::log("ERROR", "app", &format!("resolve_track_ids prepare: {e}"));
                return Vec::new();
            }
        };
        let rows = match stmt.query_map(rusqlite::params_from_iter(ids.iter().copied()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<f64>>(2)?,
            ))
        }) {
            Ok(r) => r,
            Err(e) => {
                telemetry::log("ERROR", "app", &format!("resolve_track_ids query: {e}"));
                return Vec::new();
            }
        };
        rows.filter_map(Result::ok)
            .map(|(id, path, gain)| (id, PathBuf::from(path), gain))
            .collect()
    }
}

/// Walk + hash + send for a single folder on a background thread. `on_done`
/// is invoked once when the scan finishes (success or failure). Kept as a
/// free function so `add_folder` stays under the clippy line-count limit.
fn spawn_scan_thread<F>(
    lib: mimir_core::db::Library,
    root: PathBuf,
    tx: Sender<ScanJob>,
    on_done: Option<F>,
) where
    F: FnOnce(Result<mimir_core::scanner::ScanSummary, String>) + Send + 'static,
{
    let result_root = root.clone();
    std::thread::spawn(move || {
        let conn = match lib.conn() {
            Ok(c) => c,
            Err(e) => {
                telemetry::log(
                    "ERROR",
                    "app",
                    &format!("add_folder scan: conn failed: {e}"),
                );
                if let Some(cb) = on_done {
                    cb(Err(format!("connection acquire: {e}")));
                }
                return;
            }
        };
        match mimir_core::scanner::scan_root(&conn, &root, tx) {
            Ok(summary) => {
                if summary.sent == 0 {
                    telemetry::log(
                        "WARN",
                        "app",
                        &format!(
                            "add_folder scan: no new audio files root={} walked={} hashed_fail={} known={}",
                            result_root.display(),
                            summary.walked,
                            summary.hashed_fail,
                            summary.known
                        ),
                    );
                } else {
                    telemetry::log(
                        "INFO",
                        "app",
                        &format!(
                            "add_folder scan ok root={} sent={} known={} walked={}",
                            result_root.display(),
                            summary.sent,
                            summary.known,
                            summary.walked
                        ),
                    );
                }
                if let Some(cb) = on_done {
                    cb(Ok(summary));
                }
            }
            Err(e) => {
                telemetry::log(
                    "ERROR",
                    "app",
                    &format!(
                        "add_folder scan: scan_root failed root={} err={e}",
                        result_root.display()
                    ),
                );
                if let Some(cb) = on_done {
                    cb(Err(e.to_string()));
                }
            }
        }
    });
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

#[cfg(feature = "tauri")]
fn count_files(node: &mimir_core::query::FolderNode) -> i64 {
    let mut n: i64 = node.files.len().try_into().expect("file count fits in i64");
    for c in &node.children {
        n += count_files(c);
    }
    n
}

/// LIKE-escape user input: backslash, `%`, `_` get prefixed with `\`
/// so `WHERE path LIKE '<input>%'` only matches the actual prefix.
#[cfg(feature = "tauri")]
fn escape_like_folder(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' | '%' | '_' => {
                out.push('\\');
                out.push(ch);
            }
            c => out.push(c),
        }
    }
    out
}
