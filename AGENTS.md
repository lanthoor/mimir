# AGENTS.md

Operational notes for OpenCode sessions in `mimir`. Only what an agent would
miss from filenames or a fresh `cargo build` lives here.

## Layout (Cargo workspace, resolver = "2")

- `crates/core` — `mimir-core`: library model, ingestion pipeline, SQLite (rusqlite + r2d2 pool), watcher, scanner, metadata. Owns `crates/core/migrations/0001_*.sql` .. `0010_*.sql`; run from `Library::open`.
- `crates/audio` — `mimir-audio`: decoder (`decode.rs`), transport state + queue (`transport/`), player (`player.rs`, gated on `output` feature). Pure DSP/queueing, no DB.
- `crates/app` — `mimir-app`: Tauri v2 host binary. `src/main.rs` calls `mimir_app::run()` (lib.rs); the Tauri builder, `AppState`, IPC handlers (`command.rs`), `error::AppError`, and the `mimircover://` album-cover protocol (lib.rs) live here. UI is `crates/app/ui/`: Vite 6 + React 18 + TypeScript + Tailwind + shadcn/ui (Zustand store in `src/lib/store.ts`, typed IPC in `src/lib/ipc.ts`, views in `src/views/`). Its `package.json` is `crates/app/ui/package.json`; Tauri's `beforeDevCommand`/`beforeBuildCommand` point at it via `cwd: "ui"`.
- `crates/telemetry` — `mimir-telemetry`: file-rotating logger; logs to `$XDG_STATE_HOME/var/log/mimir.log` (falls back to `~/.local/var/log/mimir.log`).

Binaries: `mimir` (host, `crates/app/src/main.rs`). Bundle config: `crates/app/tauri.conf.json` + `crates/app/capabilities/default.json`. Tauri-generated scaffolding under `crates/app/gen/` is gitignored.

## Toolchain pin (must stay in sync)

- `rust-toolchain.toml` pins `1.97.1` (current; keep `components = ["rustfmt", "clippy"]`).
- `[workspace.package].rust-version` in root `Cargo.toml` mirrors it (MSRV floor for downstream).
- Bumping Rust requires editing both files together. `rustup` reads `rust-toolchain.toml` automatically.

Workspace lints in root `Cargo.toml`: `unsafe_code = "forbid"`, `clippy::pedantic` at warn with several allows (`module_name_repetitions`, `must_use_candidate`, `missing_errors_doc`, `missing_panics_doc`, `needless_pass_by_value`, `unnecessary_wraps`). Crate `[lints] workspace = true` inherits them.

## Commands agents actually run

```bash
# Workspace-wide checks (CI runs all four; "fmt -> clippy -> test -> build" order).
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings   # warnings = errors
cargo test --workspace
cargo build --workspace

# Focused iteration
cargo test -p mimir-core                                 # single crate
cargo test -p mimir-core --lib db::tests                 # single integration-test module
cargo test -p mimir-audio --features output              # feature-gated tests
cargo build -p mimir-app --features tauri                # desktop binary (needs GTK/web deps)

# Frontend (Node 26; CI does the same in crates/app/ui)
cd crates/app/ui
npm ci && npm run lint && npm run build                  # build → crates/app/ui/dist

# Dev (Tauri shell + Vite dev server on :1420; beforeDevCommand runs `npm run dev`)
PATH=/tmp/tauri-cli/bin:$PATH cargo tauri dev --features tauri

# Release bundle (Linux; mirrors .github/workflows/bundling.yml)
cd crates/app/ui && npm ci && cd ../..
cargo build --release -p mimir-app --features tauri
cargo install --locked tauri-cli --version "^2.0" --root /tmp/tauri-cli
PATH=/tmp/tauri-cli/bin:$PATH cargo tauri build --bundles appimage deb
```

## Feature flags that change what builds

- `mimir-app` default features: `output` (which pulls `mimir-audio/output` → `rodio`). The Tauri shell is gated behind `--features tauri`.
- `mimir-audio/output` adds `rodio` for the playback queue. Without it, `Player` / `PlayerHandle` don't exist (see `#[cfg(feature = "output")]` in `crates/audio/src/lib.rs`).
- Without `--features tauri`, `mimir_app::run()` is a no-op stub that prints to stderr — so `cargo check` works on machines without GTK/webkit2gtk system deps. Linux CI installs `libwebkit2gtk-4.1-dev` + webkit/ayatana deps; runtime `.deb` depends on `libwebkit2gtk-4.1-0`.

## Linux build deps (CI installs these)

`libasound2-dev` (clippy/test/build jobs). Bundling additionally needs `libwebkit2gtk-4.1-dev`, `libxdo-dev`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `patchelf`, `xdg-utils`, `build-essential`, `file`.

## CI layout (`.github/workflows/`)

- `pr.yml` — PR to `main`. Jobs: `fmt`, `clippy`, `test`, `build`, `ui` (the five required status checks). Concurrency `pr-<n>`, cancel-in-progress.
- `ci.yml` — push to `main` + manual. Same five checks plus `cargo build --release --bin mimir` → strip → upload `mimir-linux-x86_64` artifact (1 day retention).
- `ui` jobs: Node 26 (`actions/setup-node`, pinned SHA), `npm ci` → `npm run lint` → `npm run build` in `crates/app/ui`. `bundling.yml` also runs `npm ci` before `cargo tauri build`.
- `bundling.yml` — manual + push to `main` under `crates/app/**`. Builds AppImage + .deb via `cargo tauri build`.
- All third-party GitHub Actions pinned by SHA with `# v<tag>` inline comments. `dtolnay/rust-toolchain` is given an explicit `toolchain: 1.97.1` (do not pin it by SHA in addition — it tracks the Rust release, not the channel).
- Dependabot weekly for `cargo` (`.github/dependabot.yml`) and `npm` (`/crates/app/ui`); no GitHub-Actions ecosystem entry — bump actions by hand.

## Conventions worth knowing

- TDD per commit, small self-contained commits, Conventional Commits (`feat(scope): …` / `fix(scope): …` / `refactor` / `test` / `docs` / `chore`); subject ≤ ~72 chars; explain *why* in body when the diff doesn't.
- Branch from `main`. Multi-phase work goes in as a **stack of PRs** — phase N+1 targets the phase N branch, squash-merge bottom-up.
- Workspace lints are the authority. Don't silence warnings; fix the code. `unsafe` is forbidden outright.
- Public APIs in `mimir-core` / `mimir-audio` should have doc comments (`missing_docs` is currently allow-listed — tighten as the surface grows).
- Frontend lives in `crates/app/ui` (own `package.json`, Node 26). Keep it in lockstep with CI: `npm run lint` and `npm run build` are the gates. `tauri.conf.json` `frontendDist` is **relative to `crates/app`**, so it reads `ui/dist` (not `../ui/dist`).

## App state & IPC surface

`crates/app/src/state.rs` defines `AppState` (an `Arc<Mutex<Inner>>`). Construction auto-opens the user's default library (XDG-ish via `dirs`); failure leaves `library_status.last_error` set so the SPA can recover via `library_open`. Add a new handler: implement on `AppState`, add to `invoke_handler!` in `lib.rs`, declare the `#[tauri::command]` wrapper in `command.rs`. Current surface: `library_open/status/add_folder/add_folders/remove_folder/rename_folder/rename_subdir/reveal_in_file_manager/list_folders/folder_tree/search/list_albums/list_genres/list_years/list_tracks/query_tracks/get_editable_track/update_track/clear_track_field/track_lyrics/dump_track_paths` + `audio_play/pause/resume/stop/next/previous/player_snapshot` + `app_log` + the queue family (`audio_queue_enqueue_many/play_and_enqueue_many/remove_at/move/play_at/clear/get` — the audio worker owns the pending queue, auto-advances on track-end, and the current row is pinned). **Album covers are not an IPC command** — they're served by the `mimircover://localhost/cover/{id}` custom protocol (`register_uri_scheme_protocol` in `lib.rs`); the UI builds the URL via `albumCoverUrl()` in `ui/src/lib/utils.ts`. Keep this path: never route multi-MB payloads through `invoke` (the old base64-over-IPC covers froze the main thread).

## Storage notes (mimir-core)

- `Library::open(path)` enables WAL, `foreign_keys = ON`, `busy_timeout = 5000`, then applies pending migrations from `crates/core/migrations/`. The runner is hand-rolled (`db/migrations.rs`) — **add a new `00NN_*.sql` + bump the `Migration` array at the end**; never edit applied migrations.
- Tests use `Library::in_memory()` (see `crates/core/src/db/library.rs`); they don't touch the filesystem DB.
- Search uses SQLite FTS5 with triggers keeping `track_fts` in sync; FTS triggers also backfill genre from the `track_genre` join (migration 0008). Don't `INSERT` into `track`/`album`/`artist` outside the upsert helpers in `db/upsert.rs` — the FTS row will silently go stale.

## Telemetry gotcha

`mimir_telemetry::init()` writes to `$XDG_STATE_HOME/var/log/mimir.log` (default `~/.local/var/log/mimir.log`), 5 MiB rotation, 3 generations kept. If `$XDG_STATE_HOME` and `$HOME` are both unset the logger silently degrades to stderr-only — don't assert the file exists in tests.

## Lookups (MCP only — no raw scan)

- **Code lookup in this repo → LeanKG MCP, always.** HTTP server is up (`localhost:9699/health` → ok). Prefer order: `concept_search` → `semantic_search` → `search_code` / `find_function`; then `get_context` / `get_impact_radius` / `get_dependencies`. **Never** use Grep / Glob / Read / Bash scanning to navigate the codebase. `crates/app/gen/` and `target/` are ignored by the index.
- **Library / framework / SDK / CLI / cloud API lookup → Context7 MCP, always.** `resolve-library-id` then `query-docs`. Covers Rust crates, Tauri v2, symphonia, lofty, rusqlite, etc. — use even for "well-known" APIs.
- **Never extract crates or install npm packages** to "see how it works" — query the docs (Context7) or the indexed source (LeanKG) instead. No `cargo new` subprojects, no `npm install` of a dep just to read its source.

If LeanKG HTTP is down (`curl :9699/health` fails) skip it but still **do not fall back to Grep/Glob scanning** — ask the user or read the specific file by path only.

## Pointers for more context

- Product scope: `docs/Requirements.md`. Architecture: `docs/Architecture.md`. Library/dependency rationale: `docs/TechnicalDecisions.md`. Delivery tiers: `docs/Plan.md` (Phase 0 + Tier 0 are the active scope).
- Contributing rules, TDD/stacked-PR flow, commit style: `CONTRIBUTING.md`.
- Tauri config (window size, bundle targets, linux deps): `crates/app/tauri.conf.json`. Permissions: `crates/app/capabilities/default.json`.