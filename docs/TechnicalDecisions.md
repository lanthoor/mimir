# Mimir — Technical Decisions

> Records the "why" behind each library / framework / design choice.
> Until a spike validates a pick, items are marked **Candidate** — not committed.
> Architecture implications live in [Architecture](Architecture.md); product scope in [Requirements](Requirements.md); delivery plan in [Plan](Plan.md).

> Index: [← back to Mimir](README.md)

---

## Status legend

- **Locked** — chosen, spike passed, committed.
- **Candidate** — plausible default; needs spike validation before being marked Locked.
- **Rejected** — considered and dropped (with reason).

---

## Language & runtime

### Core (Rust) — Locked

- Single static binary per platform; no runtime install needed.
- Strong type system + memory safety for an always-running watcher/audio engine.
- Best-in-class cross-platform FS/audio/watcher ecosystem.
- First-class async via `tokio` aligns with ingest/enrich/IO parallelism.

### UI (Web tech in Tauri) — Locked

- A real WebView means we keep the binary small (uses system WebView) and ship a fast, modern UI without an embedded Chromium.
- Trade-offs considered:
  - **Electron** — rejected: large bundle, two runtimes, worse startup time.
  - **Pure native (Qt / wxWidgets / egui)** — rejected for v1: lower velocity for browser-grade browsing UX.
  - **Flutter desktop** — rejected: weaker ecosystem for audio metadata + filesystem watchers in Rust.
- Tauri uses system WebView (WebKit on macOS, WebView2 on Win, WebKitGTK on Linux); keeps the install footprint small.

### UI framework — Locked

- React SPA in Tauri. The pieces below are locked (implemented in `crates/app/ui`).

---

## UI framework pieces (spike required)

### Component framework — Locked

- **React 19** (TypeScript, built with Vite, shadcn/ui components).
- Considered and dropped: Svelte, SolidJS, Vue.

### State management — Locked

- **Zustand** — single store (`crates/app/ui/src/lib/store.ts`), no boilerplate.
- Considered and dropped: Redux Toolkit, Jotai / signals.

### Server-state / IPC bridge — Locked

- Hand-rolled **typed `invoke()` wrappers** in `crates/app/ui/src/lib/ipc.ts` — one function per Rust `#[tauri::command]`, with typed payloads in `types.ts`. No query layer.
- Considered and dropped: TanStack Query, RTK Query.

### Styling — Locked

- **Tailwind CSS** + **shadcn/ui** primitives; `tailwindcss-animate` for motion.
- Theme switching (light / dark / system) via a small provider + CSS custom properties.

---

## Storage

### Primary store — Locked: SQLite

- Embedded, WAL-backed, well-understood backups, FTS5 included.
- Considered:
  - **Postgres** — rejected: requires a server, breaks "single binary, local-only" goal.
  - **LMDB / sled** — rejected as primary (kept as a possibility for transient caches only).
  - **DuckDB** — considered briefly; SQL only, no FTS5 ergonomics, rejected for primary.

### SQLite access — Locked

- `rusqlite` (bundled) + `r2d2` / `r2d2_sqlite` pool.
- Considered and dropped: `sqlx`.

### Migrations — Locked

- Hand-rolled runner in `crates/core/src/db/migrations.rs`, applying `00NN_*.sql` from `crates/core/migrations/` in order on every `Library::open`.
- Considered and dropped: `refinery`, `sqlx::migrate!`.

### Search — Locked: SQLite FTS5

- Built-in, supports the field-operator syntax we need.
- Triggers keep the FTS index in sync with the row tables.

---

## Filesystem & ingestion

### File watcher — Locked

- `notify` + `notify-debouncer-full` for cross-platform recursive events (`crates/core/src/watcher/`).

### Directory walk — Locked

- `walkdir` for the recursive scan (`crates/core/src/scanner/walk.rs`).
- Considered and dropped: `jwalk`.

### Path/content hashing — Locked

- `blake3` 32-byte content hash, dedup key `(path_hash, mtime_ns, size_bytes)`.

### Async runtime — not used

- No `tokio` / `rayon`. Scan workers and the audio worker are plain `std::thread::spawn` + `std::sync::mpsc` channels. Add an async runtime only if profiling shows we need it.

---

## Metadata extraction

### Primary library — Locked

- `lofty` — broad format coverage (ID3v1/v2, Vorbis, APE, MP4), maintained, Rust-native.
- Considered and dropped:
  - `id3` (ID3-only) — rejected: too narrow.
  - `metaflac` + `vorbis-meta` wrappers — rejected: process model and tagging write-back would suffer.

### Filename heuristics — Locked concept, implementation in code

- Pattern: `<Artist>/<Album>/<TrackNo> - <Title>.<ext>` with sensible fallbacks.
- Per-locale handling; documented test corpus in `tests/fixtures/`.

### Fingerprinting — Deferred

- `chromaprint-rs` (FFI to libchromaprint) is the planned approach; not implemented yet (Tier 4).

---

## Enrichment providers

| Provider | Decision | Notes |
|----------|----------|-------|
| AcoustID | Deferred (Tier 4) | Fingerprint → recording ID resolution. |
| MusicBrainz | Deferred (Tier 4) | Source of truth for MBIDs, release grouping. |
| Cover Art Archive | Deferred (Tier 4) | Primary source for cover art. |
| Discogs | Deferred (Tier 4) | Optional opt-in for richer releases/tags. |
| Last.fm | Deferred (Tier 4) | Bio, similar artists; also powers scrobbling. |

- All providers behind a **pluggable trait** so missing rate limits / downtime can't block playback (Tier 4).
- HTTP client candidate when implemented: `reqwest` (async) or `ureq` (sync, lighter).

---

## Audio engine

### Decoder — Locked

- `symphonia` — pure-Rust, broad format support (`crates/audio/src/decode.rs`).
- Considered and dropped: `ffmpeg-next` (libav) — licensing + binary-size cost.

### DSP — Partially locked

- ReplayGain: read from tags, applied to player volume (`audio/gain.rs`, `audio/player.rs`).
- Parametric EQ: `audio/eq.rs` prototype exists; not wired into playback (Tier 2).
- Resampling: rodio does it on the fly — `rubato` dropped.

### Audio output (cross-platform) — Locked

- **rodio** (pulls `cpal` under the hood) behind the `output` feature (`crates/audio/src/player.rs`).
- Per-platform backends come from rodio/cpal:
  - **Linux**: ALSA / PulseAudio / PipeWire.
  - **macOS**: CoreAudio.
  - **Windows**: WASAPI.

### Scrobbling — Deferred (Tier 4/6)

- `lastfm-api` (or thin `reqwest` wrapper) for Last.fm; ListenBrainz thin client — not implemented yet.

---

## Playlists

### Rules model — Locked concept

- Recursive boolean tree (`combinator + conditions`) — see [Architecture · Playlists](Architecture.md#playlists).
- Evaluation in-process over SQL result; no DSL runtime.

### Import/export formats — Locked

- M3U, M3U8, PLS, XSPF, JSPF. CSV/JSON for backup.

---

## Process model & IPC

### Process model — Locked concept

- One Tauri main process (Rust), one WebView renderer (UI), plus a Tokio worker pool and an audio thread; all share the SQLite DB.
- No secondary processes for v1; if download/cache offload becomes heavy, an auxiliary `mimir-helper` binary can be added later.

### IPC — Locked

- Tauri `invoke()` for commands; Tauri event bus (`scan:done` / `scan:error`) for scan progress; the `mimircover://` custom protocol streams album cover bytes (no base64 over IPC).

---

## Packaging & distribution

### Bundler — Locked

- `tauri build`: AppImage + .deb on Linux, .dmg on macOS (`.github/workflows/release.yml`). MSI and Flatpak still pending (Tier 6).

### Auto-update — Deferred (Tier 6)

- `tauri-plugin-updater` with signed binary manifest — not implemented yet.

### Code signing — Deferred (Tier 6)

- Apple Developer ID (notarization) and Windows Authenticode — not implemented yet.

---

## Observability & errors

### Tracing — Locked

- `crates/telemetry` (mimir-telemetry): file-rotating logger writing to `$XDG_STATE_HOME/var/log/mimir.log` (5 MiB rotation, 3 generations). Structured levels + target.
- No `db_event_log` table; scan/ingest retries are handled in-app, not persisted-as-rows.

### Error model — Locked concept

- One root `AppError` enum (`thiserror`); modules wrap lower-level errors and add context.

---

## Security & privacy

- All metadata + playback state local.
- Network calls only on user opt-in (enrichment, scrobble, update checks).
- No telemetry by default.
- Allow users to pin `no-network` mode in settings.

---

## Spike plan

Spikes were run during Tier 0/1 and each item was locked, rejected, or deferred as noted above. The remaining open work tracks the [feature checklist](Plan.md#feature-checklist) rather than this table.

| # | Spiked in Tier 0/1 | Resolution |
|---|-------|----------------------|
| S1 | React vs Svelte vs SolidJS on Tauri | **React** locked |
| S2 | Zustand / TanStack-Query ergonomics | **Zustand** + plain `invoke` wrappers locked |
| S3 | `notify` on local + network mounts | **notify + debouncer** locked |
| S4 | `lofty` coverage vs `ffmpeg-next` | **lofty** locked |
| S5 | `symphonia` cover-rate for FLAC/MP3/Opus/M4A | **symphonia** locked |
| S6 | audio backend latency + device routing | **rodio** locked (cpal underneath) |
| S7 | Chromaprint FFI build matrix | Deferred (Tier 4) |
| S8 | Tauri updater signing flow | Deferred (Tier 6) | |
