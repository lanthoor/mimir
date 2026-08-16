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
- CI matrix: Win / macOS (universal) / Linux (AppImage, .deb, Flatpak).
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
