# Mimir — Plan

> Milestones are scoped for delivery, not for the product itself.
> Product scope lives in [Requirements](Requirements.md); architecture in [Architecture](Architecture.md);
> library decisions (Locked vs Candidate) live in [Technical Decisions](TechnicalDecisions.md).

> Index: [← back to Mimir](README.md)

---

## Strategy

Build a **walking-skeleton MVP** end-to-end first — every architectural seam is real, but each tier is at its cheapest viable form. Subsequent iterations deepen each tier.

Walking skeleton (MVP-S0):
- Single crate workspace, Rust core, Tauri shell, SQLite DB.
- Watched folders + recursive watcher + idle scan.
- Tag extraction via `lofty`.
- Basic library tree: Tracks / Albums / Artists.
- Full-text search.
- One-click playback via `cpal` + `symphonia` (no DSP yet).
- Bundles per OS: MSI / .dmg / AppImage.

Each tier after S0 deepens one axis; nothing cross-cuts until a tier lands.

---

## Phase 0 — CI Bootstrap

**Goal:** green pipeline gates on a minimal Rust-only skeleton before any product code lands.

### Scope

- **Repo scaffold (pure Cargo, no Tauri yet):**
  - `Cargo.toml` — virtual workspace, `resolver = "2"`.
  - `crates/core/` — `lib.rs` exposing `pub fn hello() -> &'static str { "core" }`.
  - `crates/audio/` — same pattern.
  - `crates/app/` — `[[bin]]` with `main.rs` printing a version string from `env!("CARGO_PKG_VERSION")`.
  - Top-level `.gitignore` (Rust template) + `rust-toolchain.toml` pinning a stable toolchain.
- **Workflows under `.github/workflows/`:**
  - `pr.yml` — trigger: `pull_request` to `main`. Jobs: `fmt`, `clippy`, `test`, `build` (each a separate required status check). Concurrency group `pr-${{ github.event.pull_request.number }}`, `cancel-in-progress: true`. Permissions: `contents: read`.
  - `ci.yml` — triggers: `push` to `main`, `workflow_dispatch`. Runs the same checks, then `cargo build --release --bin mimir`, `strip` the binary, `actions/upload-artifact@v4` with `name: mimir-linux-x86_64`, `path: target/release/mimir`, `retention-days: 1`. Concurrency group `ci-main`, `cancel-in-progress: false`. Permissions: `contents: read`.
  - Both jobs pinned to `ubuntu-latest` only.
- **Branch protection:**
  - Require the four `pr.yml` checks (`fmt`, `clippy`, `test`, `build`) as required status checks on `main`.
  - Restrict dismiss/stale review to repo admins; no push (incl. force) except admins.

### Out of scope (deferred)

- Tauri shell + frontend, AppImage/.deb, MSI/.dmg.
- Windows + macOS runners.
- Signing, notarization, auto-update, release tags.
- Reproducible builds, Flatpak.

### Definition of done

- Open a PR → all four `pr.yml` jobs green; stale runs auto-cancel.
- Push to `main` → `ci.yml` produces a `mimir-linux-x86_64` artifact; previous artifacts auto-pruned within 24h.
- Branch protection on `main` enforces the four required checks.

---

## Tier 0 — Walking-skeleton MVP (S0)

**Goal:** a working desktop app that ingests, indexes, browses, and plays.

### Scope

- Repo scaffold (Cargo workspace + `app` (Tauri) + `core` + `audio`).
- Settings persisted in SQLite (watched folders).
- File watcher + on-startup full scan; debounced; idempotent.
- Embedded tag extraction; folder/filename heuristics fallback.
- Tables: `folder`, `track`, `album`, `artist`, `playlist` (static only).
- UI: Library tree (Tracks / Albums / Artists), in-memory paged lists, basic search.
- Playback: open file → PCM → output; play/pause/stop, seek, next/prev, queue.
- Bundling per OS in CI; no signing yet.
- Inline help overlay; first-run "Add folder" wizard.

### Out of scope (deferred)

- DSP (ReplayGain, EQ, crossfade).
- Smart playlists.
- Enrichment (MusicBrainz / AcoustID / cover art fetch).
- Scrobbling.
- Lyrics, statistics, plugin API.
- Auto-update.
- Code signing / notarization.

### Definition of done

- A user can: launch → add folder → see tracks fill in → search a title → double-click → hear audio.
- 1k-tag sample corpus ingests in < 30 s; UI remains responsive.
- Crash mid-scan leaves DB consistent on next launch.

---

## Tier 1 — Library depth (S1)

**Goal:** make the library feel right.

- Albums/Artists get real cover art from embedded data.
- Genre / Year / Folder / Label / Composer views.
- Faceted filters + saved searches.
- Inline + batch tag editor with revert.
- Lyrics parse + display.
- Drag-drop folder add; multi-select folder operations.

## Tier 2 — Playback quality (S2)

**Goal:** the player feels good.

- ReplayGain (track + album) with peak/RMS modes.
- Gapless playback + crossfade.
- Parametric EQ (FFT-based).
- AB repeat + speed/pitch.
- Device profile per output.
- Output backend per-OS hardening (Win WASAPI shared, macOS CoreAudio, Linux PipeWire).

## Tier 3 — Playlists (S3)

- Smart playlists (rules engine per [Architecture · Playlists](Architecture.md#playlists)).
- Import/export M3U / M3U8 / PLS / XSPF / JSPF.
- Duplicate detection on add.
- Drag-drop reorder, multi-playlist operations.

## Tier 4 — Enrichment (S4)

- Chromaprint fingerprinting.
- AcoustID lookup → MusicBrainz recording/release matching.
- Cover Art Archive fetch + cache.
- Optional Discogs / Last.fm enrichment.
- Optional write-back of canonical tags.
- All opt-in, pluggable providers behind a trait.

## Tier 5 — Listening intelligence (S5)

- Listening history with opt-out.
- Top tracks / artists / albums over time ranges.
- Library stats (formats, bitrate distribution, total duration).
- Inline insights on Now Playing.

## Tier 6 — Polish & ship (S6)

- Theming (light/dark/follow OS), density, language bundles.
- Accessibility audit (keyboard, screen reader, contrast).
- Plugin API (metadata sources, scrobblers, DSP, UI panels).
- Auto-update with signed binaries.
- Code signing + notarization for macOS, signing for Windows.
- Crash reporting (opt-in).

---

## Cross-cutting backlog (any tier)

- DB migrations pipeline + nightly snapshot.
- Backup/restore library.
- Network / no-network mode toggle.
- Background jobs rate limits + backoff.
- Multi-OS CI matrix + bundling — see [Phase 0 — CI Bootstrap](#phase-0--ci-bootstrap) for the initial Linux-only pipeline; matrix expansion and per-OS bundling land in Tier 6.
- Reproducible builds.

---

## Release gates

| Tier | Gate |
|------|------|
| S0 | Walking skeleton released as `0.1.0-alpha`, hand-tested on 2 OSes. |
| S1 | Library depth reaches parity with a typical "good desktop player". |
| S2 | Playback latency p95 < 50 ms; gapless without glitches. |
| S3 | Smart playlists tested with property-based + snapshot tests. |
| S4 | Enrichment opt-in; offline behaviour verified. |
| S5 | Listening stats match ground truth on a hand-checked window. |
| S6 | Signed installs + auto-update green on all 3 OSes. |
