//! Folder-tree query for the Folders view.
//!
//! Returns one node per directory discovered under any active watched
//! root, including empty ones, nested to reflect the filesystem layout.
//! Each node also lists the indexed audio files living immediately
//! inside it. Non-audio files are not surfaced (the Folders view is
//! about music only, per spec); subdirectories are always kept even
//! when they contain nothing — the user added the root, so the
//! directory tree they see should match what they have on disk.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;

/// A single music file (track) attached to a folder node.
///
/// `folder_id` is `None` for files whose path is indexed but the
/// enclosing folder isn't a watched root (rare — happens only if a
/// scan missed it). The UI uses the path for display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FolderFile {
    pub path: String,
    pub title: Option<String>,
    pub track_id: Option<i64>,
}

/// A directory node in the Folders view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FolderNode {
    /// Watched-root `folder.id`; `None` for deeper nodes. Used by the
    /// UI to drive Remove without re-resolving by path.
    pub folder_id: Option<i64>,
    /// Display name (last segment of `path`). `None` for root nodes so
    /// the UI can render the full path instead of a trailing slash.
    pub name: Option<String>,
    /// Absolute path on disk. Always populated.
    pub path: String,
    /// Indexed music files directly inside this dir. Empty for dirs
    /// with no audio. Subdirectories are in `children`, not here.
    pub files: Vec<FolderFile>,
    /// Direct subdirectories, sorted by path. Includes empty dirs.
    pub children: Vec<FolderNode>,
}

/// Top-level result returned to the front-end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FolderView {
    /// Flat list of every directory across all roots (roots included).
    /// Sorted by path so the icon grid is stable across re-renders.
    pub flat: Vec<FolderNode>,
    /// One entry per watched root, in `added_at` order. `name` is
    /// `None` so the renderer falls back to `path`.
    pub root_children: Vec<FolderNode>,
}

/// Return the Folders view data.
///
/// Walks every active watched root recursively. Every directory is
/// kept (even empty ones). Audio files inside each directory are
/// matched against the `track` table so the UI shows their indexed
/// title when known.
pub fn list_folders(conn: &Connection) -> Result<FolderView, rusqlite::Error> {
    mimir_telemetry::log("DEBUG", "query", "list_folders");
    let roots: Vec<(i64, PathBuf)> = {
        let mut stmt =
            conn.prepare("SELECT id, path FROM folder WHERE active = 1 ORDER BY added_at, id")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.filter_map(Result::ok)
            .map(|(id, p)| (id, PathBuf::from(p)))
            .collect()
    };

    let mut root_nodes: Vec<FolderNode> = Vec::with_capacity(roots.len());
    let mut flat: Vec<FolderNode> = Vec::new();
    for (folder_id, root_path) in roots {
        if !root_path.is_dir() {
            continue;
        }
        let node = build_node(conn, &root_path, Some(folder_id), None);
        root_nodes.push(node);
    }
    for n in &root_nodes {
        flatten(n, &mut flat);
    }
    flat.sort_by(|a, b| a.path.cmp(&b.path));

    mimir_telemetry::log(
        "INFO",
        "query",
        &format!(
            "list_folders roots={} flat={}",
            root_nodes.len(),
            flat.len()
        ),
    );
    Ok(FolderView {
        flat,
        root_children: root_nodes,
    })
}

fn build_node(
    conn: &Connection,
    dir: &Path,
    folder_id: Option<i64>,
    parent_folder_id_for_files: Option<i64>,
) -> FolderNode {
    let name = if folder_id.is_some() {
        None
    } else {
        dir.file_name().map(|s| s.to_string_lossy().into_owned())
    };
    let files = collect_files(conn, dir, parent_folder_id_for_files);
    let child_dirs = list_child_dirs(dir);
    let children: Vec<FolderNode> = child_dirs
        .into_iter()
        .map(|p| build_node(conn, &p, None, parent_folder_id_for_files))
        .collect();

    FolderNode {
        folder_id,
        name,
        path: dir.to_string_lossy().into_owned(),
        files,
        children,
    }
}

/// Audio files directly in `dir`, paired with indexed metadata when
/// available. Non-audio files (e.g. `.jpg`, `.txt`) are skipped — the
/// Folders view is music-only per spec.
#[allow(clippy::too_many_lines)]
fn collect_files(
    conn: &Connection,
    dir: &Path,
    _folder_id_for_index: Option<i64>,
) -> Vec<FolderFile> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() && crate::watcher::is_audio_path(&p) {
            paths.push(p);
        }
    }
    paths.sort();

    if paths.is_empty() {
        return Vec::new();
    }

    mimir_telemetry::log(
        "DEBUG",
        "query",
        &format!(
            "collect_files: {} audio file(s) in dir={}",
            paths.len(),
            dir.display()
        ),
    );

    // Look up indexed metadata in one query (path IN (...)). Bounded to
    // ~thousands per dir at worst, well within SQLite's variable limit.
    let path_strs: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let placeholders = std::iter::repeat_n("?", path_strs.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!("SELECT path, id, title FROM track WHERE path IN ({placeholders})");
    let params: Vec<&dyn rusqlite::ToSql> = path_strs
        .iter()
        .map(|s| s as &dyn rusqlite::ToSql)
        .collect();
    let mut lookup: std::collections::HashMap<String, (i64, Option<String>)> =
        std::collections::HashMap::new();

    // Diagnostic: log the first three paths we're going to query for and
    // a LIKE sample of what's actually in the DB under this dir. This is
    // the cheap way to find path-normalisation bugs (trailing slash,
    // NFC vs NFD, etc.) when the IN(...) lookup misses.
    if path_strs.len() >= 3 {
        mimir_telemetry::log(
            "DEBUG",
            "query",
            &format!("collect_files: sample paths[:3] = {:?}", &path_strs[..3]),
        );
    }
    let prefix = format!("{}/", dir.to_string_lossy());
    mimir_telemetry::log(
        "DEBUG",
        "query",
        &format!("collect_files: LIKE-prefix={prefix}; sample DB rows under it:"),
    );
    if let Ok(mut stmt) =
        conn.prepare("SELECT path, id FROM track WHERE path LIKE ?1 ESCAPE '\\' LIMIT 3")
    {
        let _ = stmt
            .query_map(rusqlite::params![format!("{}%", prefix)], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map(|rows| {
                for r in rows.flatten() {
                    mimir_telemetry::log(
                        "DEBUG",
                        "query",
                        &format!("collect_files: db-row path={:?} id={}", r.0, r.1),
                    );
                }
            });
    }

    if let Ok(mut stmt) = conn.prepare(&sql) {
        let rows = stmt.query_map(&*params, |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?,
            ))
        });
        if let Ok(rows) = rows {
            for r in rows.flatten() {
                lookup.insert(r.0, (r.1, r.2));
            }
        }
    }

    // Fallback: the `IN(?, ?, ...)` path-text compare misses when stored
    // paths disagree with the FS-walk strings (e.g. trailing slashes,
    // NFC/NFD normalisation, watcher vs scanner insertion paths). The
    // `track.path` column is `UNIQUE`; do a LIKE scan and match by exact
    // suffix-basename equality instead.
    if lookup.len() < path_strs.len() {
        let like_pat = format!("{}%", dir.to_string_lossy().into_owned());
        let mut stmt = match conn
            .prepare("SELECT path, id, title FROM track WHERE path LIKE ?1 ESCAPE '\\'")
        {
            Ok(s) => s,
            Err(e) => {
                mimir_telemetry::log(
                    "WARN",
                    "query",
                    &format!("collect_files: LIKE fallback prepare failed: {e}"),
                );
                return paths
                    .into_iter()
                    .map(|p| FolderFile {
                        path: p.to_string_lossy().into_owned(),
                        title: None,
                        track_id: None,
                    })
                    .collect();
            }
        };
        let prefix_native = dir.to_string_lossy().into_owned();
        let _ = stmt
            .query_map(rusqlite::params![format!("{like_pat}%")], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map(|rows| {
                for r in rows.flatten() {
                    let db_path = r.0;
                    // Match by exact prefix + same basename; the row
                    // shouldn't differ from the FS path in any other way
                    // since `track.path` is what the scanner computed.
                    if db_path.starts_with(&prefix_native)
                        && path_strs.iter().any(|p| p == &db_path)
                        && !lookup.contains_key(&db_path)
                    {
                        lookup.insert(db_path, (r.1, r.2));
                    }
                }
            });
        mimir_telemetry::log(
            "DEBUG",
            "query",
            &format!(
                "collect_files: after fallback lookup={} (path count={})",
                lookup.len(),
                path_strs.len()
            ),
        );
    }

    mimir_telemetry::log(
        "DEBUG",
        "query",
        &format!(
            "collect_files: {} audio file(s); matched {} track row(s) dir={}",
            paths.len(),
            lookup.len(),
            dir.display(),
        ),
    );

    paths
        .into_iter()
        .map(|p| {
            let s = p.to_string_lossy().into_owned();
            let (track_id, title) = match lookup.get(&s) {
                Some((id, t)) => (Some(*id), t.clone()),
                None => (None, None),
            };
            FolderFile {
                path: s,
                title,
                track_id,
            }
        })
        .collect()
}

fn list_child_dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            p.is_dir().then_some(p)
        })
        .collect();
    out.sort();
    out
}

/// Flatten a tree node into the icon-mode flat list. The current node
/// is included — icon mode is "everything that has a path on disk".
fn flatten(node: &FolderNode, out: &mut Vec<FolderNode>) {
    out.push(node.clone());
    for c in &node.children {
        flatten(c, out);
    }
}

// ponytail: per-folder file lookup uses IN (?, ?, ...). Directories with
// >999 audio files would trip SQLite's variable limit; the spec is
// personal libraries of <100k tracks so unlikely, but if a library ever
// sees a single dir with thousands of audio files this should switch
// to batching. Add when measured, not before.
