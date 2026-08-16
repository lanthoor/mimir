# Mimir — Requirements (PRD)

> Cross-platform music catalog and player.
> Desktop only (Windows / Linux / macOS). All data local by default.

> Index: [← back to Mimir](README.md)
> Related docs: [Architecture](Architecture.md) · [Technical Decisions](TechnicalDecisions.md) · [Plan](Plan.md)

---

## Goals

- Single-binary install per platform; no server required for personal use.
- Zero-config onboarding: point at folders, library is auto-built.
- Resilient: survives crashes, restarts, library moves, file renames.
- Fast navigation over libraries of 50k–500k tracks.

## Non-Goals (v1)

- Cloud sync, multi-user accounts, streaming service integration.
- Mobile companion app.
- Built-in ripping/burning.

## Users

- Collectors with large local libraries (FLAC, MP3, etc.).
- DJs / producers organizing by metadata and tags.
- Casual listeners who want smart auto-playlists.

## Functional Scope

| Area | Capabilities |
|------|--------------|
| Ingestion | Watched folders, file-watcher, rescans, multi-format, idempotent import |
| Metadata | Embedded tags, filename heuristics, fingerprinting, web enrichment |
| Browsing | Tracks / Albums / Artists / Genres / Years / Folders / Playlists |
| Search | Full-text + field operators + fuzzy |
| Playlists | Static, smart (rules), M3U/M3U8/PLS/XSPF/JSPF import/export |
| Playback | Gapless, crossfade, ReplayGain, EQ, queue, shuffle, repeat, scrobble |
| Tagging | Inline + batch edit, custom fields, lyrics |
| Statistics | Listening history, top tracks/artists, library stats |
| Extensibility | Plugin API (metadata sources, scrobblers, DSP, UI panels) |

### Ingestion

- Add/remove watched folders; persist list.
- Continuous file-watcher; startup + scheduled full rescans.
- Supported formats: MP3, FLAC, WAV, AAC/M4A, OGG/Opus, AIFF, ALAC.
- Idempotent import: dedupe by `(path, mtime, hash)`, tolerant of moves.

### Metadata & Enrichment

- Embedded tags: ID3v2, Vorbis, APE, MP4.
- Folder/filename heuristics (`Artist/Album/Track - Title.ext`).
- Audio fingerprint (Chromaprint) for identification.
- Providers (configurable, opt-in): MusicBrainz, AcoustID, Discogs, Last.fm, Cover Art Archive.
- Optional write-back of canonical tags (preserving originals).

### Browsing

- Views: Tracks, Albums, Artists, Album Artists, Genres, Composers, Years, Decades, Labels, Folders, Playlists, Smart Lists.
- Album/artist coalescing with "Various Artists" handling.
- Sort, filter, search (full-text + faceted).
- Cover art cache (embedded + sidecar + fetched).

### Playlists

- Static (manual order, drag-drop reorder).
- Smart (rules: field, operator, value; groups; limits; live update).
- Import/export: M3U, M3U8, PLS, XSPF, JSPF.

### Playback

- Gapless, crossfade, ReplayGain (track/album), EQ, AB repeat, speed/pitch.
- Queue, shuffle, repeat (one/all), continue-play.
- Output to local audio devices; volume normalization; per-device profiles.
- Last.fm / ListenBrainz scrobble (opt-in).

### Search

- Instant search across metadata fields.
- Operators: `artist:"foo" year:>2000 genre:rock -live`.
- Fuzzy match; ranked results; saved searches.

### Tagging & Editing

- Inline tag editor; batch edit; revert.
- Custom tags / dynamic fields.
- Lyrics (synced + unsynced), local cache.

### Statistics & Insights

- Listening history (opt-out).
- Top tracks/artists/albums, time ranges.
- Library stats: total duration, bitrate/format breakdown.

### Settings & Profiles

- Per-library profiles; library switching.
- Theme (light/dark/follow OS), accent, density, language.
- Keyboard shortcuts; configurable hotkeys.
- Backup/restore library DB.

### Extensibility

- Plugin API: metadata sources, scrobblers, DSP, UI panels.
- Scriptable via embedded language (JS/Lua) — v2 candidate.

## Non-Functional Requirements

- Scan ≥ 5k tracks/min; search < 50 ms p95 on 100k library; UI 60 fps.
- Idle CPU < 1%, RAM < 300 MB for 50k tracks.
- Crash-safe DB (WAL), atomic metadata writes, resume after force-kill.
- All data local by default; enrichment/scrobble opt-in.
- Single binary per OS; no external runtime install.
- i18n (UTF-8, RTL); a11y (keyboard, screen reader, high contrast).

## Platforms

- **Windows**: Win 10/11 x64; MSI installer; Media Foundation output.
- **macOS**: 12+ universal binary (Apple Silicon + Intel); CoreAudio; Hardened Runtime + notarization.
- **Linux**: x86_64 / aarch64 AppImage + .deb + Flatpak; ALSA / PulseAudio / PipeWire.

## Milestones

For delivery plan (walking-skeleton MVP + tiers S1–S6) see [Plan](Plan.md).
