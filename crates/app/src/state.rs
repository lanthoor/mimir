//! Shared state behind every IPC command.
//!
//! Holds the open `Library`, a `Transport`, and a worker handle that drains
//! `ScanJob`s into the metadata pipeline.

use std::path::Path;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

use mimir_audio::{Transport, TransportCommand};
use mimir_core::db::Library;
use mimir_core::scanner::ScanJob;

use crate::error::AppError;

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
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open (or create) the library at `path`. Idempotent.
    pub fn open_library(&self, path: &Path) -> Result<(), AppError> {
        let mut inner = self.inner.lock().expect("state poisoned");
        let lib = Library::open(path)?;
        inner.library = Some(lib);
        Ok(())
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

    pub fn transport(&self) -> Transport {
        self.inner.lock().expect("state poisoned").transport.clone()
    }

    pub fn send_transport(&self, cmd: TransportCommand) {
        let mut inner = self.inner.lock().expect("state poisoned");
        inner.transport.dispatch(cmd);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                library: None,
                transport: Transport::default(),
                scan_tx: None,
            })),
        }
    }
}
