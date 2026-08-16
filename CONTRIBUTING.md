# Contributing to Mimir

Thanks for your interest in Mimir. This document covers everything you need to
build, test, and submit changes.

## Project layout

Mimir is a Rust workspace with three crates:

- `crates/core` — library model, ingestion, metadata, DB.
- `crates/audio` — decode, DSP, output.
- `crates/app` — host binary (Tauri shell in Tier 0).

Tier 0 (the walking-skeleton MVP) and beyond is tracked in [`docs/Plan.md`](docs/Plan.md).
Product scope lives in [`docs/Requirements.md`](docs/Requirements.md),
architecture in [`docs/Architecture.md`](docs/Architecture.md),
and library/dependency decisions in [`docs/TechnicalDecisions.md`](docs/TechnicalDecisions.md).

## Toolchain

The repository pins a specific Rust version in two places — keep them in sync:

- `rust-toolchain.toml` — the toolchain contributors and CI install.
- `[workspace.package].rust-version` in `Cargo.toml` — the MSRV floor declared
  to downstream crates.

Current pinned version: **Rust 1.97.1**.

`rustup` will pick up `rust-toolchain.toml` automatically when you `cargo build`
in the repo root. No extra setup is needed beyond a working `rustup`.

The project is currently **Rust-only**. There is no Node.js, npm, or other
language toolchain in scope; if a frontend is added later (Tier 0 introduces
Tauri), a separate Node pin will be added and documented here.

## Local checks

Before opening a PR, run the same checks CI runs:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
```

The `clippy` invocation treats warnings as errors (`-D warnings`), so fix
warnings rather than silencing them. Workspace lints are configured in
`[workspace.lints]` of the root `Cargo.toml`; `unsafe_code` is `forbid` and
several `clippy::pedantic` lints are enabled at the `warn` level.

## Pull request flow

1. Branch from `main`.
2. Keep commits small and self-contained. Each logical step is one commit
   (TDD: write the failing test first, then the smallest change that makes it
   pass, then refactor).
3. Open a PR against `main`. The four required status checks (`fmt`, `clippy`,
   `test`, `build`) must pass before merge.
4. Branch protection on `main` restricts push access to repo admins and
   requires the four checks. Stale reviews are auto-dismissed only by admins.

### Stacked PRs

Multi-phase work (e.g. a tier broken into 4 phases) is submitted as a **stack
of PRs**, each one stacked on the previous:

- Phase 1 PR targets `main` directly.
- Phase 2 PR targets the Phase 1 branch (not `main`), and so on.
- Each PR in the stack must remain green on its own. Squash-merge them in
  order from bottom (Phase 1) to top, or rebase the stack onto `main` as the
  bottom PR merges.

This lets reviewers approve each phase independently while the higher phases
remain under construction.

## Commit messages

Use the [Conventional Commits](https://www.conventionalcommits.org/) style:

- `feat(scope): …` for new user-visible behaviour.
- `fix(scope): …` for bug fixes.
- `refactor(scope): …` for non-functional changes.
- `test(scope): …` for tests only.
- `docs(scope): …` for documentation.
- `chore(scope): …` for tooling, CI, dependency bumps, etc.

Keep the subject line under ~72 characters. Add a body explaining *why* when
the diff doesn't make it obvious.

## Dependency updates

- All third-party GitHub Actions in `.github/workflows/` are pinned by
  commit SHA with the tag version in an inline comment, e.g.:
  `uses: actions/checkout@<sha> # v7.0.1`.
- The pinned `dtolnay/rust-toolchain` action takes an explicit
  `toolchain: <version>` matching `rust-toolchain.toml`.
- Dependabot is the preferred mechanism for keeping actions and crates
  current; see `.github/dependabot.yml` (added in Phase 0 follow-up).

## Coding conventions

- Follow the surrounding code's style. The `fmt` and `clippy` jobs are the
  authority — when in doubt, make `cargo fmt` happy and silence `clippy`
  lints by fixing the code, not the lint.
- No `unsafe` (`unsafe_code = "forbid"`).
- Public APIs in `mimir-core` and `mimir-audio` should have doc comments;
  `missing_docs` is currently allow-listed but tighten it as the surface grows.

## License

By contributing, you agree that your contributions are dual-licensed under the
project's existing terms: MIT OR Apache-2.0 (see `Cargo.toml`).
