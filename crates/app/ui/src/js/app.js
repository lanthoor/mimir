// mimir front-end — vanilla HTML/CSS/JS wired to Tauri v2 IPC.
//
// Tauri 2 exposes `window.__TAURI__.core.invoke` for typed IPC commands.
// The library is opened implicitly by the backend on startup; the SPA
// picks up its status via `library_status` and shows a banner if the
// open failed (with a "Retry" path input).

const { invoke } = window.__TAURI__.core;

function jlog(level, ...args) {
  // Mirror to webview console + bubble to the file-backed Rust log.
  const msg = args
    .map((a) => (typeof a === "string" ? a : JSON.stringify(a)))
    .join(" ");
  // eslint-disable-next-line no-console
  console[level]("[ui-folder]", msg);
  try {
    invoke("app_log", {
      level: level.toUpperCase(),
      target: "ui-folder",
      message: msg,
    }).catch(() => {});
  } catch (_) {
    // IPC may be unavailable (e.g. plain browser preview) — ignore.
  }
}

const state = {
  view: "tracks",
  // Per-view state, restored when the user navigates back. Each key is a
  // view id; the active view is whatever's in `view`.
  perView: {
    folders: {
      folderMode: "icons", // "icons" | "tree"
      iconCwd: null, // FolderNode we're inside; null = roots
    },
    tracks: {
      query: "",
      filter: { genre: null, year: null, artistId: null, albumId: null },
    },
    albums: {},
    artists: {},
    genres: {},
    years: {},
  },
  folderTree: { flat: [], root_children: [] },
  items: [],
  library: { path: null, last_error: null },
  // Last user-action result (separate from library-open errors so a
  // successful folder add doesn't get reported as "library not open").
  // kind: null | "ok" | "err".
  action: { kind: null, text: null },
  // album_id → data: URL string (or "" when no cover). Cached so the
  // WebView doesn't re-fetch on every render.
  covers: {},
  // Track id most recently double-clicked → drives "now playing" + lyrics.
  nowPlayingTrackId: null,
};

const $list = document.getElementById("list");
const $search = document.getElementById("search");
const $nav = document.querySelectorAll("nav button[data-view]");
const $addFolder = document.getElementById("add-folder");
const $addFolderDialog = document.getElementById("add-folder-dialog");
const $addFolderPath = document.getElementById("add-folder-path");
const $addFolderBrowse = document.getElementById("add-folder-browse");
const $addFolderInfo = document.getElementById("add-folder-info");
const $status = document.getElementById("status");
const $actionStatus = document.getElementById("action-status");
const $editDialog = document.getElementById("edit-track-dialog");
const $editForm = document.getElementById("edit-track-form");

const $prev = document.getElementById("prev");
const $play = document.getElementById("play");
const $pause = document.getElementById("pause");
const $stop = document.getElementById("stop");
const $next = document.getElementById("next");
const $npTitle = document.getElementById("np-title");
const $npArtist = document.getElementById("np-artist");
const $lyricsToggle = document.getElementById("lyrics-toggle");
const $lyricsDialog = document.getElementById("lyrics-dialog");
const $lyricsContent = document.getElementById("lyrics-content");

function vstate() {
  return state.perView[state.view] || {};
}

function setCwd(node) {
  const vs = vstate();
  vs.iconCwd = node == null ? null : node.path;
  jlog("info", "setCwd", {
    view: state.view,
    nodeName: node?.name ?? null,
    nodePath: node?.path ?? null,
    childrenLen: node?.children?.length ?? 0,
    filesLen: node?.files?.length ?? 0,
  });
}

// Resolve the persisted cwd string back into a live node after refresh.
// Returns null when the path is gone (folder removed) or we're at roots.
function resolveCwd() {
  const path = vstate().iconCwd;
  if (path == null) return null;
  for (const r of state.folderTree.root_children) {
    const m = findByPath(r, path);
    if (m) return m;
  }
  jlog("warn", "resolveCwd: no match for path", {
    path,
    roots: state.folderTree.root_children.map((r) => r.path),
  });
  return null;
}

function findByPath(node, path) {
  if (node.path === path) return node;
  for (const c of node.children || []) {
    if (findByPath(c, path)) return c;
  }
  return null;
}

function render() {
  jlog("debug", "render:enter", { view: state.view, folderMode: vstate().folderMode, iconCwd: vstate().iconCwd });
  const vs = vstate();
  syncSearchFromView();
  $list.className = "list " + state.view + (
    state.view === "folders"
      ? " folders-" + (vs.folderMode || "icons")
      : ""
  );
  if (state.view === "folders") {
    renderFoldersView();
  } else {
    $list.replaceChildren(...state.items.map(renderCard));
  }
  renderFoldersTools();
  renderStatus();
  renderFilterChips();
}

function renderFoldersTools() {
  const $tools = document.getElementById("folders-tools");
  if (!$tools) return;
  const visible = state.view === "folders";
  $tools.hidden = !visible;
  const mode = vstate().folderMode || "icons";
  for (const btn of $tools.querySelectorAll("[data-folder-mode]")) {
    btn.classList.toggle("active", btn.dataset.folderMode === mode);
  }
}

function renderFoldersView() {
  const vs = vstate();
  jlog("debug", "renderFoldersView:enter", {
    view: state.view,
    folderMode: vs.folderMode,
    iconCwd: vs.iconCwd,
    rootCount: state.folderTree.root_children.length,
  });
  $list.replaceChildren();
  if ((vs.folderMode || "icons") === "tree") {
    for (const root of state.folderTree.root_children) {
      $list.append(renderFolderTree(root, true));
    }
    if (state.folderTree.root_children.length === 0) {
      $list.append(renderFoldersEmpty());
    }
    return;
  }
  renderFolderBreadcrumb();
  const cwd = resolveCwd();
  const nodes = cwd == null ? state.folderTree.root_children : cwd.children;
  const files = cwd == null ? [] : (cwd.files || []);
  jlog("debug", "renderFoldersView:icon", {
    cwdPath: cwd?.path ?? null,
    childrenCount: nodes.length,
    filesCount: files.length,
    childrenNames: nodes.map((n) => n.name ?? n.path),
  });
  if (state.folderTree.root_children.length === 0) {
    $list.append(renderFoldersEmpty());
    return;
  }
  if (nodes.length === 0 && files.length === 0 && vs.iconCwd == null) {
    $list.append(renderFoldersEmpty());
    return;
  }
  if (cwd != null) {
    $list.append(renderIconUpButton());
  }
  if (nodes.length === 0 && files.length === 0) {
    const empty = document.createElement("div");
    empty.className = "folders-empty";
    empty.textContent = "This folder is empty.";
    $list.append(empty);
    return;
  }
  // Subdirs first, then files — so the grid reads as a file-explorer
  // "folders above, files below".
  for (const node of nodes) {
    $list.append(renderFolderIcon(node));
  }
  for (const file of files) {
    $list.append(renderFolderFileIcon(file));
  }
}

function renderFolderBreadcrumb() {
  const $bar = document.getElementById("folders-breadcrumb");
  if (!$bar) return;
  $bar.replaceChildren();
  const vs = vstate();
  if ((vs.folderMode || "icons") !== "icons" || state.view !== "folders") {
    $bar.hidden = true;
    return;
  }
  const trail = computeIconTrail();
  if (trail.length <= 1) {
    $bar.hidden = true;
    return;
  }
  $bar.hidden = false;
  for (let i = 0; i < trail.length; i++) {
    if (i > 0) {
      const sep = document.createElement("span");
      sep.className = "bc-sep";
      sep.textContent = "\u203A";
      $bar.append(sep);
    }
    const seg = document.createElement("button");
    seg.type = "button";
    seg.className = "crumb";
    seg.textContent = i === 0 ? "Folders" : (trail[i].name ?? trail[i].path);
    seg.addEventListener("click", () => navigateIconTo(i));
    $bar.append(seg);
  }
}

function computeIconTrail() {
  const cwd = resolveCwd();
  if (cwd == null) return [];
  const all = [
    ...state.folderTree.root_children,
  ];
  const out = [];
  let cur = cwd;
  out.push(cur);
  while (true) {
    const parent = all.find((n) =>
      (n.children || []).some((c) => c.path === cur.path),
    );
    if (!parent) break;
    out.unshift(parent);
    cur = parent;
  }
  return out;
}

function findDescendant(node, pred) {
  if (pred(node)) return true;
  for (const c of node.children || []) {
    if (findDescendant(c, pred)) return true;
  }
  return false;
}

function navigateIconTo(index) {
  const trail = computeIconTrail();
  if (index === 0 || trail.length === 0) {
    setCwd(null);
  } else {
    setCwd(trail[index - 1]);
  }
  render();
}

function iconUp() {
  const trail = computeIconTrail();
  // trail = [root, ..., current]. Going up one level means landing on
  // the second-to-last entry (or roots if current is the root).
  if (trail.length <= 1) {
    setCwd(null);
  } else {
    setCwd(trail[trail.length - 2]);
  }
  render();
}

function renderFoldersEmpty() {
  const empty = document.createElement("div");
  empty.className = "folders-empty";
  empty.textContent =
    "No folders with tracks yet. Click + folder to watch a directory.";
  return empty;
}

function renderFolderIcon(node) {
  const card = document.createElement("div");
  card.className = "card folder-icon";
  card.classList.add("folder-icon-clickable");
  jlog("debug", "renderFolderIcon", {
    name: node.name,
    path: node.path,
    children: (node.children || []).length,
    files: (node.files || []).length,
  });
  card.addEventListener("dblclick", () => {
    jlog("info", "folder dblclick", {
      name: node.name,
      path: node.path,
      children: (node.children || []).length,
      files: (node.files || []).length,
    });
    setCwd(node);
    render();
  });
  card.addEventListener("contextmenu", (ev) => {
    ev.preventDefault();
    openContextMenu(ev, folderContextMenuItems(node));
  });
  const icon = document.createElement("div");
  icon.className = "folder-icon-glyph";
  icon.textContent = "\u{1F4C1}"; // 📁
  const title = document.createElement("div");
  title.className = "title";
  title.textContent = node.name ?? fileBaseName(node.path);
  const subtitle = document.createElement("div");
  subtitle.className = "subtitle";
  subtitle.textContent = `${node.path}`;
  card.append(icon, title, subtitle);
  return card;
}

function renderFolderFileIcon(file) {
  jlog("debug", "renderFolderFileIcon", {
    path: file.path,
    title: file.title,
    track_id: file.track_id,
  });
  const card = document.createElement("div");
  card.className = "card file-icon";
  card.title = file.path;
  const icon = document.createElement("div");
  icon.className = "file-icon-glyph";
  icon.textContent = "\u{1F3B5}"; // 🎵
  const title = document.createElement("div");
  title.className = "title";
  title.textContent = file.title ?? fileBaseName(file.path);
  const subtitle = document.createElement("div");
  subtitle.className = "subtitle";
  subtitle.textContent = fileBaseName(file.path);
  card.append(icon, title, subtitle);
  card.addEventListener("contextmenu", (ev) => {
    ev.preventDefault();
    openContextMenu(ev, fileContextMenuItems(file));
  });
  if (file.track_id != null) {
    card.addEventListener("dblclick", () => {
      jlog("info", "file dblclick", {
        path: file.path,
        track_id: file.track_id,
      });
      state.nowPlayingTrackId = file.track_id;
      $npTitle.textContent = file.title ?? "(untitled)";
      $npArtist.textContent = "";
      invoke("audio_play", { trackId: file.track_id }).catch((e) => {
        jlog("error", "audio_play failed", { msg: String(e), track_id: file.track_id });
      });
    });
  } else {
    jlog("debug", "file has no track_id; dblclick disabled", { path: file.path });
  }
  return card;
}

function renderIconUpButton() {
  const card = document.createElement("div");
  card.className = "card folder-icon folder-icon-up";
  // dblclick only — single-click would steal the first click of every
  // double-click on neighbouring cards, sending the user back to roots.
  card.addEventListener("dblclick", iconUp);
  const icon = document.createElement("div");
  icon.className = "folder-icon-glyph";
  icon.textContent = "\u2191";
  const title = document.createElement("div");
  title.className = "title";
  title.textContent = "..";
  const subtitle = document.createElement("div");
  subtitle.className = "subtitle";
  subtitle.textContent = "Up to Folders";
  card.append(icon, title, subtitle);
  return card;
}

function fileBaseName(p) {
  const segs = p.split(/[\\/]/);
  return segs[segs.length - 1] || p;
}

function renderFolderTree(node, isRoot) {
  const wrap = document.createElement("div");
  wrap.className = "folder-tree-node" + (isRoot ? " folder-tree-root" : "");
  const row = document.createElement("div");
  row.className = "folder-tree-row";
  const caret = document.createElement("span");
  caret.className = "folder-tree-caret";
  const hasChildren =
    (node.children && node.children.length > 0) || node.files.length > 0;
  caret.textContent = hasChildren ? "\u25BE" : "\u00A0\u00A0";
  const name = document.createElement("span");
  name.className = "folder-tree-name";
  name.textContent = node.name ?? fileBaseName(node.path);
  const meta = document.createElement("span");
  meta.className = "folder-tree-meta";
  meta.textContent = `${node.files.length} files`;
  row.append(caret, name, meta);
  const subPath = document.createElement("div");
  subPath.className = "folder-tree-subpath";
  subPath.textContent = node.path;
  row.addEventListener("contextmenu", (ev) => {
    ev.preventDefault();
    openContextMenu(ev, folderContextMenuItems(node));
  });
  wrap.append(row, subPath);
  if (hasChildren) {
    const children = document.createElement("div");
    children.className = "folder-tree-children";
    for (const f of node.files) {
      children.append(renderTreeFile(f));
    }
    for (const c of node.children) {
      children.append(renderFolderTree(c, false));
    }
    wrap.append(children);
    // Collapsible: clicking the caret toggles a `.collapsed` class.
    caret.style.cursor = "pointer";
    caret.addEventListener("click", () => {
      children.classList.toggle("collapsed");
      caret.textContent = children.classList.contains("collapsed")
        ? "\u25B8"
        : "\u25BE";
    });
  }
  return wrap;
}

function renderTreeFile(file) {
  const li = document.createElement("div");
  li.className = "folder-tree-file";
  li.textContent = "\u{1F3B5} " + (file.title ?? fileBaseName(file.path));
  li.title = file.path;
  li.addEventListener("contextmenu", (ev) => {
    ev.preventDefault();
    openContextMenu(ev, fileContextMenuItems(file));
  });
  if (file.track_id != null) {
    li.addEventListener("dblclick", () => {
      state.nowPlayingTrackId = file.track_id;
      $npTitle.textContent = file.title ?? "(untitled)";
      $npArtist.textContent = "";
      invoke("audio_play", { trackId: file.track_id }).catch(console.error);
    });
  }
  return li;
}

function openContextMenu(ev, items) {
  const $m = document.getElementById("context-menu");
  $m.replaceChildren();
  for (const it of items) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "context-item";
    btn.textContent = it.label;
    btn.disabled = it.disabled === true;
    if (!btn.disabled) {
      btn.addEventListener("click", () => {
        closeContextMenu();
        it.onClick();
      });
    }
    $m.append(btn);
  }
  $m.hidden = false;
  $m.style.left = `${ev.clientX}px`;
  $m.style.top = `${ev.clientY}px`;

  // Close the menu on any click outside, or Escape.
  setTimeout(() => {
    window.addEventListener("click", closeContextMenu, { once: true });
    window.addEventListener(
      "keydown",
      (e) => {
        if (e.key === "Escape") closeContextMenu();
      },
      { once: true },
    );
  }, 0);
}

function closeContextMenu() {
  const $m = document.getElementById("context-menu");
  $m.hidden = true;
}

function folderContextMenuItems(node) {
  const items = [
    {
      label: "Open",
      onClick: () => {
        setCwd(node);
        render();
      },
    },
    {
      label: "Rename",
      onClick: async () => {
        if (node.folder_id != null) {
          // Watched root — full path rename.
          const newPath = prompt("New folder path", node.path);
          if (!newPath || newPath === node.path) return;
          try {
            await invoke("library_rename_folder", {
              folderId: node.folder_id,
              newPath: newPath.trim(),
            });
            state.action = { kind: "ok", text: `Renamed to ${newPath}` };
            renderAction();
            await refresh();
          } catch (e) {
            const msg = e?.message ?? e?.toString() ?? JSON.stringify(e);
            state.action = { kind: "err", text: `Rename failed: ${msg}` };
            renderAction();
          }
        } else {
          // Subdir — single segment rename on disk.
          const oldName = node.name ?? fileBaseName(node.path);
          const newName = prompt("New folder name", oldName);
          if (!newName || newName === oldName) return;
          try {
            await invoke("library_rename_subdir", {
              currentPath: node.path,
              newName: newName.trim(),
            });
            state.action = { kind: "ok", text: `Renamed to ${newName}` };
            renderAction();
            await refresh();
          } catch (e) {
            const msg = e?.message ?? e?.toString() ?? JSON.stringify(e);
            state.action = { kind: "err", text: `Rename failed: ${msg}` };
            renderAction();
          }
        }
      },
    },
  ];
  if (node.folder_id != null) {
    items.push({
      label: "Remove",
      onClick: async () => {
        if (!confirm(`Stop watching this folder?\n\n${node.path}`)) return;
        try {
          await invoke("library_remove_folder", { folderId: node.folder_id });
          state.action = { kind: "ok", text: `Removed folder ${node.path}` };
          renderAction();
          await refresh();
        } catch (e) {
          const msg = e?.message ?? e?.toString() ?? JSON.stringify(e);
          state.action = { kind: "err", text: `Remove failed: ${msg}` };
          renderAction();
        }
      },
    });
  }
  return items;
}

function fileContextMenuItems(file) {
  const canPlay = file.track_id != null;
  return [
    {
      label: "Play",
      disabled: !canPlay,
      onClick: () => {
        if (!canPlay) return;
        state.nowPlayingTrackId = file.track_id;
        $npTitle.textContent = file.title ?? "(untitled)";
        $npArtist.textContent = "";
        invoke("audio_play", { trackId: file.track_id }).catch((e) => {
          jlog("error", "audio_play failed", {
            msg: String(e),
            track_id: file.track_id,
          });
        });
      },
    },
    {
      label: "Reveal in file manager",
      onClick: async () => {
        try {
          await invoke("library_reveal_in_file_manager", { path: file.path });
        } catch (e) {
          const msg = e?.message ?? e?.toString() ?? JSON.stringify(e);
          state.action = { kind: "err", text: `Reveal failed: ${msg}` };
          renderAction();
        }
      },
    },
    {
      label: "Edit",
      disabled: !canPlay,
      onClick: () => {
        if (!canPlay) return;
        openTrackEditor(file.track_id);
      },
    },
  ];
}

function renderFilterChips() {
  const $bar = document.getElementById("filter-bar");
  if (!$bar) return;
  $bar.replaceChildren();
  const filter = vstate().filter;
  if (!filter) {
    $bar.hidden = true;
    return;
  }
  const active = Object.entries(filter).filter(([, v]) => v != null);
  if (state.view !== "tracks" || active.length === 0) {
    $bar.hidden = true;
    return;
  }
  $bar.hidden = false;
  for (const [key, value] of active) {
    const chip = document.createElement("button");
    chip.className = "chip";
    chip.type = "button";
    chip.textContent = `${key}=${value} ✕`;
    chip.addEventListener("click", () => {
      vstate().filter[key] = null;
      refresh();
    });
    $bar.append(chip);
  }
}

function renderStatus() {
  const { path, last_error } = state.library;
  if (!last_error) {
    $status.hidden = true;
    $status.replaceChildren();
    return;
  }
  $status.hidden = false;
  $status.replaceChildren();
  const msg = document.createElement("div");
  msg.textContent = "Library could not be opened:";
  const code = document.createElement("code");
  code.textContent = last_error;
  const pathInfo = document.createElement("div");
  pathInfo.append(document.createTextNode("at "), document.createElement("code"));
  pathInfo.lastChild.textContent = path ?? "(unknown)";
  $status.append(msg, code, pathInfo);
  renderAction();
}

function renderAction() {
  const { kind, text } = state.action;
  if (!kind || !text) {
    $actionStatus.hidden = true;
    $actionStatus.replaceChildren();
    return;
  }
  $actionStatus.hidden = false;
  $actionStatus.className = `action-status ${kind}`;
  $actionStatus.textContent = text;
}

function renderCard(item) {
  const card = document.createElement("div");
  card.className = "card";
  const title = document.createElement("div");
  title.className = "title";
  const subtitle = document.createElement("div");
  subtitle.className = "subtitle";

  if (state.view === "tracks") {
    title.textContent = item.title ?? "(untitled)";
    subtitle.textContent =
      [item.artist_name, item.album_title].filter(Boolean).join(" — ") || item.path;
    card.addEventListener("dblclick", () => {
      state.nowPlayingTrackId = item.id;
      $npTitle.textContent = item.title ?? "(untitled)";
      $npArtist.textContent = item.artist_name ?? "";
      refreshLyricsButton(item.id);
      invoke("audio_play", { trackId: item.id });
    });
    card.addEventListener("contextmenu", (ev) => {
      ev.preventDefault();
      openTrackEditor(item.id);
    });
  } else if (state.view === "albums") {
    title.textContent = item.title;
    subtitle.textContent =
      [item.artist_name, item.track_count + " tracks"].filter(Boolean).join(" — ");
    const cover = state.covers[item.id];
    if (cover) {
      const img = document.createElement("img");
      img.className = "cover";
      img.alt = "";
      img.src = cover;
      card.append(img);
    }
  } else if (state.view === "genres") {
    title.textContent = item.name;
    subtitle.textContent = `${item.track_count} tracks`;
    card.addEventListener("click", () => {
      state.view = "tracks";
      $nav.forEach((b) => b.classList.toggle("active", b.dataset.view === "tracks"));
      const tracks = state.perView.tracks;
      tracks.filter = { genre: item.name, year: null, artistId: null, albumId: null };
      refresh();
    });
  } else if (state.view === "years") {
    title.textContent = String(item.year);
    subtitle.textContent = `${item.track_count} tracks`;
    card.addEventListener("click", () => {
      state.view = "tracks";
      $nav.forEach((b) => b.classList.toggle("active", b.dataset.view === "tracks"));
      const tracks = state.perView.tracks;
      tracks.filter = { genre: null, year: item.year, artistId: null, albumId: null };
      refresh();
    });
  } else {
    title.textContent = item.name;
    subtitle.textContent = "";
  }

  card.append(title, subtitle);
  return card;
}

async function refresh() {
  jlog("debug", "refresh:enter", { view: state.view });
  try {
    if (state.view === "tracks") {
      const tracks = state.perView.tracks;
      const filter = tracks.filter || {};
      const hasFilter = Object.values(filter).some((v) => v != null);
      const q = (tracks.query || "").trim();
      // Three-way: faceted query > text search > plain list. FTS rejects
      // an empty query so we fall through to list_tracks for the no-input
      // case (otherwise the Tracks view is blank until you type).
      if (hasFilter) {
        state.items = await invoke("library_query_tracks", {
          genre: filter.genre,
          year: filter.year,
          artistId: filter.artistId,
          albumId: filter.albumId,
          limit: 100,
          offset: 0,
        });
      } else if (q.length > 0) {
        state.items = await invoke("library_search", { query: q, limit: 100 });
      } else {
        state.items = await invoke("library_list_tracks", { limit: 100, offset: 0 });
      }
    } else if (state.view === "albums") {
      const albums = await invoke("library_list_albums", { limit: 200, offset: 0 });
      state.items = albums;
      // Fetch covers in parallel; cache by album.id, render after.
      const missing = albums.filter((a) => !(a.id in state.covers));
      await Promise.all(
        missing.map(async (a) => {
          const resp = await invoke("library_album_cover", { albumId: a.id });
          if (resp) {
            const [mime, bytes] = resp;
            const bytesArray = Array.isArray(bytes) ? new Uint8Array(bytes) : new Uint8Array();
            let binary = "";
            for (let i = 0; i < bytesArray.length; i++) binary += String.fromCharCode(bytesArray[i]);
            state.covers[a.id] = `data:${mime};base64,${btoa(binary)}`;
          } else {
            state.covers[a.id] = "";
          }
        }),
      );
    } else if (state.view === "genres") {
      state.items = await invoke("library_list_genres");
    } else if (state.view === "years") {
      state.items = await invoke("library_list_years");
    } else if (state.view === "folders") {
      state.folderTree = await invoke("library_folder_tree");
      // Drop cwd if it points at a path that no longer exists in the
      // freshly-fetched tree (e.g. the user removed that watched root).
      const target = vstate().iconCwd;
      if (target != null && resolveCwd() == null) {
        vstate().iconCwd = null;
      }
    } else {
      // Artists are not yet exposed via IPC; show empty state.
      state.items = [];
    }
    render();
  } catch (e) {
    console.error("library refresh failed:", e);
  }
}

async function refreshStatus() {
  try {
    state.library = await invoke("library_status");
  } catch (e) {
    console.error("library_status failed:", e);
    state.library = { path: null, last_error: e?.message ?? String(e) };
  }
  render();
}

$search.addEventListener("input", (ev) => {
  const tracks = state.perView.tracks;
  tracks.query = ev.target.value;
  refresh();
});

// Re-sync the search box when the user comes back to the Tracks tab so
// they see what they typed last time. The "input" listener above updates
// `tracks.query` while we're in Tracks; this restores the input value
// when the view changes.
function syncSearchFromView() {
  if (state.view === "tracks") {
    $search.value = state.perView.tracks.query || "";
  } else {
    // Empty elsewhere to avoid leaking track-search terms into other views.
    $search.value = "";
  }
}

$nav.forEach((btn) => {
  btn.addEventListener("click", () => {
    jlog("info", "nav click", { view: btn.dataset.view, prevView: state.view });
    state.view = btn.dataset.view;
    $nav.forEach((b) => b.classList.toggle("active", b === btn));
    refresh();
  });
});

for (const btn of document.querySelectorAll("[data-folder-mode]")) {
  btn.addEventListener("click", () => {
    const vs = vstate();
    if ((vs.folderMode || "icons") === btn.dataset.folderMode) return;
    vs.folderMode = btn.dataset.folderMode;
    setCwd(null); // modes show the same nodes differently
    render();
  });
}

$addFolder.addEventListener("click", () => {
  if (state.library.last_error) {
    alert("Library isn't open. Fix the open error above first.");
    return;
  }
  $addFolderPath.value = "";
  $addFolderDialog.showModal();
});

$addFolderDialog.addEventListener("close", async () => {
  if ($addFolderDialog.returnValue !== "default") return;
  const path = $addFolderPath.value.trim();
  if (!path) return;
  state.action = { kind: null, text: null };
  renderAction();
  try {
    const result = await invoke("library_add_folder", { path });
    const s = result.summary || {};
    console.log(
      `library_add_folder: ${path} -> folder_id=${result.folder_id} ` +
        `walked=${s.walked} sent=${s.sent} known=${s.known} hashed_fail=${s.hashed_fail}`,
    );
    state.action = {
      kind: "ok",
      text: `Added ${s.sent} new tracks from ${path} (skipped ${s.known} known, ${s.hashed_fail} unreadable).`,
    };
    renderAction();
    await refresh();
  } catch (e) {
    console.error("add_folder failed:", e);
    const msg = (e && (e.message ?? e?.toString())) || JSON.stringify(e);
    state.action = { kind: "err", text: `Add folder failed: ${msg}` };
    renderAction();
  }
});

if ($addFolderBrowse) {
  $addFolderBrowse.addEventListener("click", async () => {
    // tauri-plugin-dialog exposes `open` via window.__TAURI__.dialog.
    const dialog = window.__TAURI__ && window.__TAURI__.dialog;
    if (!dialog) {
      showInfo("Folder picker unavailable in this build.", "err");
      return;
    }
    try {
      const picked = await dialog.open({
        directory: true,
        multiple: false,
        title: "Select a music folder",
      });
      if (typeof picked === "string" && picked.length > 0) {
        $addFolderPath.value = picked;
        showInfo(
          `Picked ${picked}. Click "Add" to scan.`,
          "ok",
        );
      }
    } catch (e) {
      console.error("folder picker failed:", e);
      showInfo(
        `Folder picker failed: ${e?.message ?? JSON.stringify(e)}`,
        "err",
      );
    }
  });
}

function showInfo(text, kind) {
  if (!$addFolderInfo) return;
  $addFolderInfo.textContent = text;
  $addFolderInfo.classList.remove("ok", "err", "empty");
  if (kind) $addFolderInfo.classList.add(kind);
}

// Reset the info bar whenever the dialog opens.
$addFolderDialog.addEventListener("show", () => showInfo("", null));
$addFolderPath.addEventListener("input", () => showInfo("", null));

async function openTrackEditor(trackId) {
  try {
    const fields = await invoke("library_get_editable_track", { trackId });
    $editForm.elements["title"].value = fields.title ?? "";
    $editForm.elements["genre"].value = fields.genre ?? "";
    $editForm.elements["year"].value = fields.year ?? "";
    $editForm.elements["track_no"].value = fields.track_no ?? "";
    $editForm.elements["disc_no"].value = fields.disc_no ?? "";
    $editForm.dataset.trackId = String(trackId);
    $editDialog.showModal();
  } catch (e) {
    console.error("get_editable_track failed:", e);
    alert(`Open editor failed: ${e?.message ?? e}`);
  }
}

$editDialog.addEventListener("close", async () => {
  if ($editDialog.returnValue !== "save") return;
  const trackId = Number($editForm.dataset.trackId);
  const data = new FormData($editForm);
  const strOrNull = (name) => {
    const v = String(data.get(name) ?? "").trim();
    return v === "" ? null : v;
  };
  const intOrNull = (name) => {
    const v = strOrNull(name);
    if (v == null) return null;
    const n = Number(v);
    return Number.isFinite(n) ? Math.trunc(n) : null;
  };
  // To clear a field, push its name into `clear`. To set it, include the
  // (possibly null) value. The Rust side translates this into the
  // `Option<Option<T>>` semantic.
  const clearedKeys = ["title", "genre", "year", "track_no", "disc_no"];
  const patch = {
    title: strOrNull("title"),
    genre: strOrNull("genre"),
    year: intOrNull("year"),
    track_no: intOrNull("track_no"),
    disc_no: intOrNull("disc_no"),
    clear: [],
  };
  // Clear semantics: empty input clears the field.
  for (const k of clearedKeys) {
    if (patch[k] == null) patch.clear.push(k);
  }
  try {
    await invoke("library_update_track", { trackId, patch });
    await refresh();
  } catch (e) {
    console.error("update_track failed:", e);
    alert(`Save failed: ${e?.message ?? e}`);
  }
});

$play.addEventListener("click", () => {
  // The toolbar play button restarts the most recently double-clicked
  // track. Sending `trackId: 0` was a no-op (track_id=0 doesn't exist in
  // the library) — the toolbar now does nothing until a track is chosen.
  if (state.nowPlayingTrackId != null) {
    invoke("audio_play", { trackId: state.nowPlayingTrackId }).catch(console.error);
  }
});
$pause.addEventListener("click", () => invoke("audio_pause").catch(console.error));
$stop.addEventListener("click", () => invoke("audio_stop").catch(console.error));
$next.addEventListener("click", () => invoke("audio_next").catch(console.error));
$prev.addEventListener("click", () => invoke("audio_previous").catch(console.error));

async function refreshLyricsButton(trackId) {
  try {
    const lyrics = await invoke("library_track_lyrics", { trackId });
    if (lyrics) {
      $lyricsToggle.hidden = false;
      $lyricsToggle.dataset.trackId = String(trackId);
    } else {
      $lyricsToggle.hidden = true;
      delete $lyricsToggle.dataset.trackId;
    }
  } catch (e) {
    console.error("lyrics lookup failed:", e);
    $lyricsToggle.hidden = true;
  }
}

$lyricsToggle.addEventListener("click", async () => {
  const trackId = Number($lyricsToggle.dataset.trackId);
  if (!trackId) return;
  try {
    const lyrics = await invoke("library_track_lyrics", { trackId });
    if (!lyrics) return;
    $lyricsContent.textContent = lyrics.text;
    $lyricsDialog.showModal();
  } catch (e) {
    console.error("lyrics fetch failed:", e);
  }
});

// Tauri v2 emits a window-level drag-drop event when files land on the
// webview. We only accept directories.
if (window.__TAURI__ && window.__TAURI__.window) {
  const { getCurrentWindow } = window.__TAURI__.window;
  getCurrentWindow().onDragDropEvent(async (event) => {
    if (event.payload.type !== "drop") return;
    const paths = (event.payload.paths || []).filter(
      (p) => typeof p === "string" && p.length > 0,
    );
    if (paths.length === 0) return;
    try {
      await invoke("library_add_folders", { paths });
      await refresh();
    } catch (e) {
      console.error("drop-add folders failed:", e);
      alert(`Add folders failed: ${e?.message ?? e}`);
    }
  });
}

// Initial paint (empty) + status check + first listing.
render();
refreshStatus();
refresh().catch((e) => console.error("initial refresh failed:", e));
