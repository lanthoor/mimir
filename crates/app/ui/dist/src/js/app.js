// mimir front-end — vanilla HTML/CSS/JS wired to Tauri v2 IPC.
//
// Tauri 2 exposes `window.__TAURI__.core.invoke` for typed IPC commands.
// Until the user clicks "+ folder" and the server-side scanner has indexed
// something, the library list is empty.

const { invoke } = window.__TAURI__.core;

const state = {
  view: "tracks",
  query: "",
  items: [],
};

const $list = document.getElementById("list");
const $search = document.getElementById("search");
const $nav = document.querySelectorAll("nav button[data-view]");
const $addFolder = document.getElementById("add-folder");
const $addFolderDialog = document.getElementById("add-folder-dialog");
const $addFolderPath = document.getElementById("add-folder-path");
const $nowPlayingTitle = document.getElementById("np-title");
const $nowPlayingArtist = document.getElementById("np-artist");

const $prev = document.getElementById("prev");
const $play = document.getElementById("play");
const $pause = document.getElementById("pause");
const $stop = document.getElementById("stop");
const $next = document.getElementById("next");

function render() {
  $list.className = "list " + state.view;
  $list.replaceChildren(...state.items.map(renderCard));
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
    } else {
      // Albums / artists are not yet exposed via IPC; show empty state.
      state.items = [];
    }
    render();
  } catch (e) {
    console.error("library refresh failed:", e);
  }
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

// Initial paint (empty).
render();
