# mimir front-end

The front-end is a **React 19 + TypeScript** SPA built with **Vite**, styled
with **Tailwind CSS** and **shadcn/ui**, and holds its state in a single
**Zustand** store (`src/lib/store.ts`). It calls Tauri v2 IPC commands through
typed wrappers in `src/lib/ipc.ts`.

Tauri builds it via `npm run build` (output in `ui/dist`) and serves that
directory as the frontend — see `crates/app/tauri.conf.json`
(`frontendDist: "ui/dist"`, `beforeDevCommand`/`beforeBuildCommand`).

## Layout

```
ui/
├── src/
│   ├── lib/          # store.ts (Zustand), ipc.ts (typed invoke), types.ts
│   ├── hooks/        # use-library, use-scan-events
│   ├── views/        # Tracks / Albums / Artists / Genres / Years / Folders / Queue
│   ├── components/   # nav, now-playing, dialogs, shadcn/ui primitives
│   ├── App.tsx       # shell: Nav + view router + Now Playing
│   └── main.tsx      # entry
├── vite.config.ts
├── tailwind.config.js
└── package.json
```

## Commands (from `ui/`)

```bash
npm ci            # install deps
npm run dev       # Vite dev server on :1420 (used by `cargo tauri dev`)
npm run lint      # eslint .
npm run build     # tsc -b && vite build → ui/dist
```

## Frontend → Rust contract

`ipc.ts` wraps one `invoke()` per Rust `#[tauri::command]` in
`crates/app/src/command.rs`. Non-exhaustive map:

| Frontend `ipc.<fn>` | Rust `#[tauri::command]` |
|---------------------|--------------------------|
| `libraryOpen(path)` | `library_open(path: String)` |
| `libraryAddFolder(path)` | `library_add_folder(path: String)` |
| `libraryAddFolders(paths)` | `library_add_folders(paths: Vec<String>)` |
| `librarySearch(query, limit)` | `library_search(query: String, limit: Option<i64>)` |
| `audioPlay(trackId)` | `audio_play(track_id: i64)` |
| `audioPause()` | `audio_pause()` |
| `audioResume()` | `audio_resume()` |
| `audioStop()` | `audio_stop()` |
| `audioNext()` | `audio_next()` |
| `audioPrevious()` | `audio_previous()` |

Album covers are **not** IPC commands — they're served by the
`mimircover://localhost/cover/{id}` custom protocol (registered in
`crates/app/src/lib.rs`); the UI builds the URL via `albumCoverUrl()` in
`src/lib/utils.ts`.
