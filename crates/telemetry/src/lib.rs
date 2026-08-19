//! Tiny file-rotating logger.
//!
//! Writes to `~/.local/var/log/mimir.log` (XDG-ish). When the active file
//! exceeds 5 MiB it's renamed to `mimir.log.<n>.old` and a fresh
//! `mimir.log` is opened. We keep up to 3 generations; older files are
//! dropped on rotation.
//!
//! Two entry points:
//! - `init()` installs a global default and returns a [`Guard`] that
//!   flushes on Drop.
//! - `log(...)` writes one line to both stderr and the active log
//!   file without taking a dependency on a logging facade.
//!
//! ponytail: single-thread file writer guarded by a `Mutex`. Throughput
//! is fine for desktop usage; if volume balloons, swap for a real
//! `tracing-appender` later.

#[cfg(test)]
#[path = "rotation_tests.rs"]
mod rotation;
#[cfg(test)]
mod tests;

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const MAX_BYTES: u64 = 5 * 1024 * 1024;
const KEEP_GENERATIONS: usize = 3;

static STATE: OnceLock<Mutex<Option<State>>> = OnceLock::new();

struct State {
    dir: PathBuf,
    file: File,
    path: PathBuf,
    bytes: u64,
}

/// Drop guard. Drop semantics flush the buffer; nothing fancier here.
pub struct Guard;

fn cell() -> &'static Mutex<Option<State>> {
    STATE.get_or_init(|| Mutex::new(None))
}

fn default_log_dir() -> Option<PathBuf> {
    // Prefer $XDG_STATE_HOME; fall back to ~/.local/var.
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local")));
    let dir = base?.join("var").join("log");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Open the log directory + active file. Idempotent; safe to call twice.
pub fn init() -> Option<Guard> {
    let dir = default_log_dir()?;
    let mut guard = cell().lock().expect("log mutex poisoned");
    if guard.is_some() {
        return Some(Guard);
    }
    let path = dir.join("mimir.log");
    let file = match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("mimir-log: cannot open {}: {e}", path.display());
            return None;
        }
    };
    let bytes = file.metadata().map_or(0, |m| m.len());
    *guard = Some(State {
        dir,
        file,
        path,
        bytes,
    });
    Some(Guard)
}

/// Append one line. Prefixes a timestamp if the writer is initialised; if
/// not, falls back to stderr-only.
pub fn log(level: &str, target: &str, message: &str) {
    let ts = current_timestamp();
    let line = format!("[{ts}] [{level}] [{target}] {message}\n");

    let mut guard = cell().lock().expect("log mutex poisoned");
    match guard.as_mut() {
        Some(state) => {
            if let Err(e) = write_line(state, &line) {
                eprintln!("mimir-log: write failed: {e}");
                eprint!("{line}");
            } else {
                eprint!("{line}");
            }
        }
        None => {
            // No init yet — best-effort to stderr.
            eprint!("{line}");
        }
    }
}

fn write_line(state: &mut State, line: &str) -> io::Result<()> {
    let len = u64::try_from(line.len()).unwrap_or(u64::MAX);
    if state.bytes + len > MAX_BYTES {
        rotate(state)?;
    }
    state.file.write_all(line.as_bytes())?;
    state.bytes += len;
    state.file.flush()?;
    Ok(())
}

fn rotate(state: &mut State) -> io::Result<()> {
    state.file.flush()?;
    drop(std::mem::replace(&mut state.file, empty_file()));

    // Shift generations: .1.old -> drop, .2.old -> drop, .3.old -> drop,
    // .log -> .1.old, .log -> empty.
    for n in (1..=KEEP_GENERATIONS).rev() {
        let target = state.dir.join(format!("mimir.log.{n}.old"));
        if n == KEEP_GENERATIONS {
            let _ = fs::remove_file(&target);
        } else {
            let src = state.dir.join(format!("mimir.log.{}.old", n + 1));
            let _ = fs::rename(&src, &target);
        }
    }
    let first = state.dir.join("mimir.log.1.old");
    let _ = fs::rename(&state.path, &first);

    state.file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&state.path)?;
    state.bytes = 0;
    Ok(())
}

fn empty_file() -> File {
    File::open(Path::new("/dev/null")).unwrap_or_else(|_| {
        // Last-ditch: an in-memory write sink. Should never hit.
        File::create_new("/tmp/mimir-log-fallback").expect("create fallback log")
    })
}

/// `YYYY-MM-DD HH:MM:SS` UTC timestamp. Cheap; doesn't depend on `chrono`.
fn current_timestamp() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Days since 1970-01-01, plus today's seconds-into-day.
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let time = secs % 86_400;
    let hour = time / 3600;
    let minute = (time % 3600) / 60;
    let second = time % 60;

    // Civil-from-days algorithm by Howard Hinnant.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_096;
    let doe = u64::try_from(z - era * 146_096).unwrap_or(0);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = i64::try_from(yoe).unwrap_or(0) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let m = u32::try_from(m).unwrap_or(1);
    let year = if m <= 2 { y + 1 } else { y };

    format!("{year:04}-{m:02}-{d:02} {hour:02}:{minute:02}:{second:02}")
}
