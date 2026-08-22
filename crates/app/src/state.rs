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

    /// Enqueue a folder for scanning. The actual scan is async — this
    /// command returns once the folder row is upserted. A scan worker drains
    /// the channel on a background thread.
    pub fn add_folder(
        &self,
        root: &Path,
    ) -> Result<(i64, mimir_core::scanner::ScanSummary), AppError> {
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

        // Spawn a worker on first call.
        let mut inner = self.inner.lock().expect("state poisoned");
        if inner.scan_tx.is_none() {
            let (tx, rx) = channel::<ScanJob>();
            let worker_lib = lib.clone();
            let target = root.to_path_buf();
            std::thread::spawn(move || {
                telemetry::log(
                    "INFO",
                    "app",
                    &format!("scan worker thread spawned target={}", target.display()),
                );
                mimir_core::metadata::run_worker(&worker_lib.conn().expect("conn"), rx);
            });
            inner.scan_tx = Some(tx);
            telemetry::log("INFO", "app", "scan_tx initialised");
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
        match mimir_core::scanner::scan_root(&conn, root, tx) {
            Ok(summary) => {
                if summary.sent == 0 {
                    telemetry::log(
                        "WARN",
                        "app",
                        &format!(
                            "add_folder scanned but found no audio files root={} walked={} hashed_fail={} known={}",
                            root.display(),
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
                            "add_folder ok folder_id={folder_id} root={} sent={} known={} walked={}",
                            root.display(),
                            summary.sent,
                            summary.known,
                            summary.walked
                        ),
                    );
                }
                Ok((folder_id, summary))
            }
            Err(e) => {
                telemetry::log(
                    "ERROR",
                    "app",
                    &format!(
                        "add_folder scan_root failed folder_id={folder_id} root={} err={e}",
                        root.display()
                    ),
                );
                Err(e.into())
            }
        }
    }

    /// Add multiple folders in one go. Each is upserted by path so re-adding
    /// the same root is a no-op. Workers drain scan jobs on a shared thread.
    pub fn add_folders<I, P>(
        &self,
        paths: I,
    ) -> Result<Vec<(i64, mimir_core::scanner::ScanSummary)>, AppError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        telemetry::log("INFO", "app", "add_folders enter batch");
        let mut out: Vec<(i64, mimir_core::scanner::ScanSummary)> = Vec::new();
        let mut n = 0u64;
        for p in paths {
            n += 1;
            match self.add_folder(p.as_ref()) {
                Ok(r) => out.push(r),
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
            // Prefer album gain over track gain; pass None when neither exists.
            let gain = album_db.or(track_db);
            player
                .handle()
                .send(PlayerCommand::SetReplayGainDb(gain))
                .map_err(|e| {
                    telemetry::log(
                        "ERROR",
                        "app",
                        &format!("play_track: SetReplayGainDb send err: {e}"),
                    );
                    AppError::from(e)
                })?;
            player
                .handle()
                .send(PlayerCommand::Play(path.clone()))
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
