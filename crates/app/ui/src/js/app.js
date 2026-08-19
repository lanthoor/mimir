// mimir front-end — vanilla HTML/CSS/JS wired to Tauri v2 IPC.
//
// Tauri 2 exposes `window.__TAURI__.core.invoke` for typed IPC commands.
// The library is opened implicitly by the backend on startup; the SPA
// picks up its status via `library_status` and shows a banner if the
// open failed (with a "Retry" path input).

const { invoke } = window.__TAURI__.core;

const state = {
  view: "tracks",
  query: "",
  items: [],
  library: { path: null, last_error: null },
  // album_id → data: URL string (or "" when no cover). Cached so the
  // WebView doesn't re-fetch on every render.
  covers: {},
  // Active facet filters when in 'tracks' view. Any non-null value narrows
  // the listing via library_query_tracks.
  filter: { genre: null, year: null, artistId: null, albumId: null },
  // Track id most recently double-clicked → drives "now playing" + lyrics.
  nowPlayingTrackId: null,
};

const $list = document.getElementById("list");
const $search = document.getElementById("search");
const $nav = document.querySelectorAll("nav button[data-view]");
const $addFolder = document.getElementById("add-folder");
const $addFolderDialog = document.getElementById("add-folder-dialog");
const $addFolderPath = document.getElementById("add-folder-path");
const $status = document.getElementById("status");
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

function render() {
  $list.className = "list " + state.view;
  $list.replaceChildren(...state.items.map(renderCard));
  renderStatus();
  renderFilterChips();
}

function renderFilterChips() {
  const $bar = document.getElementById("filter-bar");
  if (!$bar) return;
  $bar.replaceChildren();
  const active = Object.entries(state.filter).filter(([, v]) => v != null);
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
      state.filter[key] = null;
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
      state.filter = { genre: item.name, year: null, artistId: null, albumId: null };
      state.view = "tracks";
      $nav.forEach((b) => b.classList.toggle("active", b.dataset.view === "tracks"));
      refresh();
    });
  } else if (state.view === "years") {
    title.textContent = String(item.year);
    subtitle.textContent = `${item.track_count} tracks`;
    card.addEventListener("click", () => {
      state.filter = { genre: null, year: item.year, artistId: null, albumId: null };
      state.view = "tracks";
      $nav.forEach((b) => b.classList.toggle("active", b.dataset.view === "tracks"));
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
  try {
    if (state.view === "tracks") {
      const hasFilter =
        Object.values(state.filter).some((v) => v != null);
      state.items = hasFilter
        ? await invoke("library_query_tracks", {
            genre: state.filter.genre,
            year: state.filter.year,
            artistId: state.filter.artistId,
            albumId: state.filter.albumId,
            limit: 100,
            offset: 0,
          })
        : await invoke("library_search", {
            query: state.query,
            limit: 100,
          });
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
  state.query = ev.target.value;
  refresh();
});

$nav.forEach((btn) => {
  btn.addEventListener("click", () => {
    state.view = btn.dataset.view;
    $nav.forEach((b) => b.classList.toggle("active", b === btn));
    refresh();
  });
});

$addFolder.addEventListener("click", () => {
  if (state.library.last_error) {
    alert("Library isn't open. Fix the open error above first.");
    return;
  }
  $addFolderPath.value = "";
  $addFolderDialog.showModal();
});

$addFolderDialog.addEventListener("close", async () => {
  if ($addFolderDialog.returnValue === "default") {
    const path = $addFolderPath.value.trim();
    if (!path) return;
    try {
      await invoke("library_add_folder", { path });
      await refresh();
    } catch (e) {
      console.error("add_folder failed:", e);
      alert(`Add folder failed: ${e?.message ?? e}`);
    }
  }
});

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

$play.addEventListener("click", () => invoke("audio_play", { trackId: 0 }).catch(console.error));
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

// Initial paint (empty) + status check.
render();
refreshStatus();
