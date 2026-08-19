//! Filesystem watcher process.
//!
//! Spawns a background thread that runs `notify-debouncer-full`, watches
//! `roots` recursively, and forwards translated `IngestEvent`s to `tx`.

use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};

use mimir_telemetry as telemetry;
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
    telemetry::log(
        "INFO",
        "watcher",
        &format!("spawn_watcher start root={}", root.display()),
    );
    let mut debouncer = new_debouncer(
        Duration::from_millis(500),
        None,
        move |res: DebounceEventResult| {
            match res {
                Ok(events) => {
                    let n = events.len();
                    telemetry::log("DEBUG", "watcher", &format!("recv debounce batch n={n}"));
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
                                telemetry::log(
                                    "DEBUG",
                                    "watcher",
                                    &format!("skip non-audio path={}", raw.display()),
                                );
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
                                other => {
                                    telemetry::log(
                                        "DEBUG",
                                        "watcher",
                                        &format!("unhandled kind {other:?} path={}", raw.display()),
                                    );
                                    continue;
                                }
                            };
                            match tx.send(IngestEvent {
                                path: raw.clone(),
                                kind: kind.clone(),
                            }) {
                                Ok(()) => telemetry::log(
                                    "INFO",
                                    "watcher",
                                    &format!("emitted {kind:?} path={}", raw.display()),
                                ),
                                Err(e) => telemetry::log(
                                    "ERROR",
                                    "watcher",
                                    &format!("tx.send failed path={} err={e}", raw.display()),
                                ),
                            }
                        }
                    }
                }
                Err(errors) => {
                    for e in errors {
                        telemetry::log("ERROR", "watcher", &format!("{e:?}"));
                    }
                }
            }
        },
    )?;

    debouncer.watcher().watch(root, RecursiveMode::Recursive)?;
    telemetry::log(
        "INFO",
        "watcher",
        &format!(
            "watcher armed root={} backend={} debounce=500ms",
            root.display(),
            std::any::type_name::<notify::RecommendedWatcher>()
        ),
    );

    Ok(WatcherHandle {
        _debouncer: debouncer,
    })
}

fn is_audio_path_str(path: &Path) -> bool {
    super::event::is_audio_path(path)
}
