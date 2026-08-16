//! Mapping from `notify` events to our domain `IngestEvent`s.

use notify::event::{ModifyKind, RenameMode};
use notify::Event;

use super::event::{is_audio_path, EventKind, IngestEvent};

/// Map a single `notify` event to at most one `IngestEvent`.
///
/// Returns `None` if the event should be ignored (non-audio path, or an event
/// kind that doesn't translate). Renames are special — `notify-debouncer-full`
/// already pairs `From`+`To` into a single `DebouncedEvent` carrying both
/// sides; the `from_rename` field is set on the `To` event by the debouncer
/// when applicable.
pub fn to_ingest(event: &Event) -> Option<IngestEvent> {
    to_ingest_pair(None, event)
}

/// Map a `notify` event plus an optional paired "from" rename event into a
/// single `IngestEvent`. Use this for rename events that arrive as a pair.
pub fn to_ingest_pair(from_rename: Option<&Event>, event: &Event) -> Option<IngestEvent> {
    let path = event.paths.first()?;

    if let Some(from) = from_rename {
        if let Some(from_path) = from.paths.first() {
            if matches!(notify_kind(event), NotifyKind::RenameTo) {
                return Some(IngestEvent {
                    path: path.clone(),
                    kind: EventKind::Renamed {
                        from: from_path.clone(),
                        to: path.clone(),
                    },
                });
            }
        }
    }

    if !is_audio_path(path) {
        return None;
    }

    let kind = match notify_kind(event) {
        NotifyKind::Create => EventKind::Created,
        NotifyKind::Modify => EventKind::Modified,
        NotifyKind::Remove => EventKind::Removed,
        NotifyKind::Other | NotifyKind::RenameFrom | NotifyKind::RenameTo => return None,
    };
    Some(IngestEvent {
        path: path.clone(),
        kind,
    })
}

/// Coarse classification of a `notify::EventKind` for our purposes.
#[derive(Debug, Clone, Copy)]
enum NotifyKind {
    Create,
    Modify,
    Remove,
    RenameFrom,
    RenameTo,
    Other,
}

fn notify_kind(event: &Event) -> NotifyKind {
    use notify::event::EventKind as K;
    match &event.kind {
        K::Create(_) => NotifyKind::Create,
        K::Modify(ModifyKind::Name(RenameMode::From)) => NotifyKind::RenameFrom,
        K::Modify(ModifyKind::Name(RenameMode::To)) => NotifyKind::RenameTo,
        K::Modify(_) => NotifyKind::Modify,
        K::Remove(_) => NotifyKind::Remove,
        _ => NotifyKind::Other,
    }
}
