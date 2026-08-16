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

### UI framework — Candidate

- Lean React SPA in Tauri; specifics (state, data layer, styling) are decided below as Candidates and must be spiked.

---

## UI framework pieces (spike required)

### Component framework — Candidate

- Default candidate: **React**.
- Considered: Svelte, SolidJS, Vue. Decision pending build-size / DX spike.

### State management — Candidate

- Default candidates: local component state + lightweight store; alternatives evaluated:
  - **Zustand** — small, no boilerplate.
  - **Redux Toolkit** — rejected for v1 unless scale demands it.
  - **Jotai / signals** — viable, decide during spike.

### Server-state / IPC bridge — Candidate

- Default candidate: **TanStack Query** over Tauri `invoke()` commands.
- Considered: raw `fetch`-style wrappers, RTK Query.
- Spike must verify streaming large lists (track/playlists) and binary payload (audio frames) paths.

### Styling — Candidate

- Preference for CSS layers (or vanilla-extract / Tailwind); finalize during spike based on theme-switching needs.

---

## Storage

### Primary store — Locked: SQLite

- Embedded, WAL-backed, well-understood backups, FTS5 included.
- Considered:
  - **Postgres** — rejected: requires a server, breaks "single binary, local-only" goal.
  - **LMDB / sled** — rejected as primary (kept as a possibility for transient caches only).
  - **DuckDB** — considered briefly; SQL only, no FTS5 ergonomics, rejected for primary.

### SQLite access — Candidate

- `rusqlite` (bundled) + `r2d2_sqlite` pool, OR `sqlx`.
- Decision criterion: ergonomics for prepared-statement reuse and migrations.

### Migrations — Candidate

- `refinery` vs `sqlx::migrate!` vs hand-rolled — pick during spike.

### Search — Locked: SQLite FTS5

- Built-in, supports the field-operator syntax we need.
- Triggers keep the FTS index in sync with the row tables.

---

## Filesystem & ingestion

### File watcher — Candidate

- `notify` + `notify-debouncer-full` for cross-platform recursive events.
- Spike must validate behavior on network mounts (SMB/NFS) and removable media.

### Directory walk — Candidate

- `walkdir` (parallel scan via Rayon).
- Alternative: `jwalk` — keep in pocket if walk performance bottlenecks.

### Path/content hashing — Candidate

- `blake3` for content hashes; SHA-256 acceptable if any tooling wants it.
- Dedup key: `(path_hash, mtime_ns, size_bytes)` to handle rename + replace cheaply.

### Async runtime — Candidate

- `tokio` for the worker pool.
- `rayon` for CPU-bound parallel scans (DB writes, hashing).

---

## Metadata extraction

### Primary library — Candidate

- `lofty` — broad format coverage (ID3v1/v2, Vorbis, APE, MP4), maintained, Rust-native.
- Alternatives:
  - `id3` (ID3-only) — rejected: too narrow.
  - `metaflac` + `vorbis-meta` wrappers — rejected: process model and tagging write-back would suffer.

### Filename heuristics — Locked concept, implementation in code

- Pattern: `<Artist>/<Album>/<TrackNo> - <Title>.<ext>` with sensible fallbacks.
- Per-locale handling; documented test corpus in `tests/fixtures/`.

### Fingerprinting — Candidate

- `chromaprint-rs` (FFI to libchromaprint) to compute AcoustID-compatible fingerprints.
- Build-time feature to disable for binaries that don't want the FFI footprint.

---

## Enrichment providers

| Provider | Decision | Notes |
|----------|----------|-------|
| AcoustID | Candidate (locked-in concept) | Fingerprint → recording ID resolution. |
| MusicBrainz | Candidate | Source of truth for MBIDs, release grouping. |
| Cover Art Archive | Candidate | Primary source for cover art. |
| Discogs | Candidate | Optional opt-in for richer releases/tags. |
| Last.fm | Candidate | Bio, similar artists; also powers scrobbling. |

- All providers behind a **pluggable trait** so missing rate limits / downtime can't block playback.
- HTTP client candidates: `reqwest` (async), `ureq` (sync, lighter). Decision during spike.

---

## Audio engine

### Decoder — Candidate

- `symphonia` — pure-Rust, broad format support, frame-accurate output.
- Considered: `ffmpeg-next` (libav) — rejected for v1 due to licensing + binary-size cost; revisit if Symphonia can't cover a required format.

### DSP — Candidate

- `rubato` for resampling.
- `realfft` or `rustfft` for EQ (FFT-based parametric EQ).
- `eq-rs` (or in-tree biquad cascade) for the EQ coefficient math.
- ReplayGain via `vgmstream`-style RVA / RG tag parsing; no FFI.

### Audio output (cross-platform) — Candidate

- `cpal` for the unified API.
- Per-platform backend preferences:
  - **Linux**: ALSA (low latency) preferred where available; PulseAudio / PipeWire via `cpal`.
  - **macOS**: CoreAudio via `cpal` (no extra FFI).
  - **Windows**: WASAPI via `cpal`; WASAPI exclusive mode is a v2 enhancement.

### Scrobbling — Candidate

- `lastfm-api` (or thin `reqwest` wrapper) for Last.fm.
- ListenBrainz: thin `reqwest` client.

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

### IPC — Candidate

- Tauri `invoke()` for commands; Tauri channels/event bus for streaming large lists and progress.
- Streaming binary (audio frames) → dedicated `tauri-plugin-fs` or a custom channel — spike it.

---

## Packaging & distribution

### Bundler — Candidate

- `tauri build` for MSI (Win), `.dmg` (mac), AppImage/.deb (Linux).
- Spike to confirm Linux Flatpak readiness (sandbox + filesystem access for watched folders).

### Auto-update — Candidate

- `tauri-plugin-updater` with signed binary manifest.

### Code signing — Candidate

- Apple Developer ID (notarization) and Windows Authenticode EV/OV.
- Track budget and cert renewal in project ops docs.

---

## Observability & errors

### Tracing — Candidate

- `tracing` + `tracing-subscriber` for structured logs.
- Persisted `db_event_log` for retry-eligible error categories.

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

Each spike ends with one of: **Lock**, **Reject**, **Re-spike**.

| # | Spike | Decision it unblocks |
|---|-------|----------------------|
| S1 | React vs Svelte vs SolidJS build-size + DX on Tauri | UI framework |
| S2 | Zustand / Jotai / TanStack-Query ergonomics on Tauri | state + data-layer libs |
| S3 | `notify` on SMB / NFS / ext4 / exFAT | watcher confirm |
| S4 | `lofty` coverage vs `ffmpeg-next` for ALAC,Opus,AAC | metadata + decoder |
| S5 | `symphonia` cover-rate for FLAC/MP3/Opus/M4A | decoder |
| S6 | `cpal` latency + device routing on each OS | audio output strategy |
| S7 | Chromaprint FFI build matrix on Win/macOS/Linux | fingerprint feature flag |
| S8 | Tauri updater signing flow | release pipeline |
