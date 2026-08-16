//! Filesystem watcher process.
//!
//! Spawns a background thread that runs `notify-debouncer-full`, watches
//! `roots` recursively, and forwards translated `IngestEvent`s to `tx`.

use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};

use super::event::IngestEvent;

/// Handle to a running watcher. Dropping the handle stops it.
pub struct WatcherHandle {
    _debouncer: Debouncer<notify::RecommendedWatcher, FileIdMap>,
}

/// Spawn a recursive watcher on `root` and forward `IngestEvent`s to `tx`.
///
/// The returned `WatcherHandle` keeps the watcher alive; drop it to stop.
pub fn spawn_watcher(root: &Path, tx: Sender<IngestEvent>) -> Result<WatcherHandle, notify::Error> {
    let mut debouncer = new_debouncer(
        Duration::from_millis(500),
        None,
        move |res: DebounceEventResult| {
            match res {
                Ok(events) => {
                    for debounced in events {
                        // A single DebouncedEvent may carry a paired rename
                        // (From + To). notify-debouncer-full collapses them into
                        // one event with `event.kind = Modify(Name(To))` and a
                        // non-empty `event.paths[0]`; the original "from" lives
                        // nowhere accessible here, so renames degrade to a
                        // plain `Modified` on the new path. P4/P5 wiring will
                        // re-scan by path when that matters.
                        for raw in &debounced.event.paths {
                            if !is_audio_path_str(raw) {
                                continue;
                            }
                            let kind = match &debounced.event.kind {
                                notify::event::EventKind::Create(_) => {
                                    super::event::EventKind::Created
                                }
                                notify::event::EventKind::Modify(_) => {
                                    super::event::EventKind::Modified
                                }
                                notify::event::EventKind::Remove(_) => {
                                    super::event::EventKind::Removed
                                }
                                _ => continue,
                            };
                            let _ = tx.send(IngestEvent {
                                path: raw.clone(),
                                kind,
                            });
                        }
                    }
                }
                Err(errors) => {
                    for e in errors {
                        eprintln!("watcher error: {e:?}");
                    }
                }
            }
        },
    )?;

    debouncer.watcher().watch(root, RecursiveMode::Recursive)?;

    Ok(WatcherHandle {
        _debouncer: debouncer,
    })
}

fn is_audio_path_str(path: &Path) -> bool {
    super::event::is_audio_path(path)
}
