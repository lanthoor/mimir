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
};

const $list = document.getElementById("list");
const $search = document.getElementById("search");
const $nav = document.querySelectorAll("nav button[data-view]");
const $addFolder = document.getElementById("add-folder");
const $addFolderDialog = document.getElementById("add-folder-dialog");
const $addFolderPath = document.getElementById("add-folder-path");
const $status = document.getElementById("status");

const $prev = document.getElementById("prev");
const $play = document.getElementById("play");
const $pause = document.getElementById("pause");
const $stop = document.getElementById("stop");
const $next = document.getElementById("next");

function render() {
  $list.className = "list " + state.view;
  $list.replaceChildren(...state.items.map(renderCard));
  renderStatus();
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
    card.addEventListener("dblclick", () => invoke("audio_play", { trackId: item.id }));
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
  } else if (state.view === "years") {
    title.textContent = String(item.year);
    subtitle.textContent = `${item.track_count} tracks`;
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
      state.items = await invoke("library_search", {
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

$play.addEventListener("click", () => invoke("audio_play", { trackId: 0 }).catch(console.error));
$pause.addEventListener("click", () => invoke("audio_pause").catch(console.error));
$stop.addEventListener("click", () => invoke("audio_stop").catch(console.error));
$next.addEventListener("click", () => invoke("audio_next").catch(console.error));
$prev.addEventListener("click", () => invoke("audio_previous").catch(console.error));

// Initial paint (empty) + status check.
render();
refreshStatus();
