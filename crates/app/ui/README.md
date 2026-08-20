# mimir front-end

The front-end is a vanilla HTML/CSS/JS SPA that calls Tauri v2 IPC commands
directly via `window.__TAURI__.core.invoke`. **No build step is required.**

Tauri loads `ui/src/` directly — edit files there, no copy step.

## Layout

```
ui/
└── src/         # what you edit, what Tauri loads
    ├── index.html
    ├── styles/app.css
    └── js/app.js
```

## Why no bundler?

- One file per concern. No node_modules, no transpilation, no Svelte 5 runes
  to debug. The browser support matrix is "anything WebKitGTK 4.1+ supports",
  which is far wider than `"chrome105"`.
- Reviewers can read the entire front-end in one sitting.
- Replacing the front-end with a Svelte/React/Vue SPA later is a non-event:
  point Tauri at the new build's output directory.

## Frontend → Rust contract

| Frontend `invoke(name, args)` | Rust `#[tauri::command]` |
|------------------------------|--------------------------|
| `library_open(path)` | `library_open(path: String)` |
| `library_add_folder(path)` | `library_add_folder(path: String)` |
| `library_search(query, limit)` | `library_search(query: String, limit: Option<i64>)` |
| `audio_play(trackId)` | `audio_play(track_id: i64)` |
| `audio_pause()` | `audio_pause()` |
| `audio_resume()` | `audio_resume()` |
| `audio_stop()` | `audio_stop()` |
| `audio_next()` | `audio_next()` |
| `audio_previous()` | `audio_previous()` |
