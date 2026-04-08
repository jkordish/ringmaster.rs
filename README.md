# ringmaster.rs

`ringmaster.rs` is a local-first Rust terminal application for exploring Oura Cloud data with a Ratatui interface, SQLite-backed local storage, deterministic demo mode, and a poll-first Oura Cloud API v2 integration.

## Status

This repository now includes a real phase-1 foundation:

- `clap` CLI with `tui`, `doctor`, `auth login`, `sync once`, and `demo`
- `ratatui + crossterm + tokio` application shell
- deterministic demo data that exercises Dashboard, Timeline, Trends, and Ops
- SQLite migrations and typed store/query boundaries
- real loopback OAuth login with server-side code exchange, PKCE, and CSRF-safe state handling
- persisted auth/session metadata in SQLite with token secrets stored through the OS keyring seam
- real poll-first sync for personal info, daily summaries, and heartrate into normalized tables plus raw payload cache
- capability-aware live UI states for missing scopes, stale or empty data, and last sync errors
- structured logging via `tracing`

The project is intentionally not feature-complete yet. The goal is a trustworthy local foundation with one real vertical slice, not a one-shot full product dump.

## Commands

```bash
cargo run -- tui
cargo run -- doctor
cargo run -- auth login
cargo run -- sync once
cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase1
cargo run -- demo
```

Rust toolchain baseline: `rust-version = 1.88`.

Behavior notes:

- `ringmaster tui` launches the live TUI when attached to a terminal. Without a TTY it renders a snapshot of the current local app state instead.
- `ringmaster demo` launches the same UI shell in deterministic demo mode. Without a TTY it renders a text snapshot, which is useful for CI and screenshot-oriented workflows.
- `ringmaster doctor` resolves paths, initializes SQLite, applies migrations, and prints auth, capability, sync, and path diagnostics.
- `ringmaster auth login` starts a loopback OAuth flow, validates state, exchanges the code server-side, and persists auth/session metadata locally.
- `ringmaster sync once` refreshes auth when needed, imports the phase-1 Oura slice, caches raw payloads, and upserts normalized SQLite rows.
- `ringmaster sync once --dry-run --fixture-dir tests/fixtures/phase1` exercises the same normalization pipeline without live credentials or database writes.

## Local-first layout

`ringmaster.rs` uses XDG-friendly paths by default:

- config directory: `$XDG_CONFIG_HOME/ringmaster` or `~/.config/ringmaster`
- config file: `config.toml`
- state directory: `$XDG_STATE_HOME/ringmaster` or `~/.local/state/ringmaster`
- database: `ringmaster.db`
- cache directory: `$XDG_CACHE_HOME/ringmaster` or `~/.cache/ringmaster`

The app creates runtime directories as needed, but it does not create a config file unless you do.

## Example config

Create `~/.config/ringmaster/config.toml`:

```toml
[logging]
filter = "ringmaster=debug"

[oura]
client_id = "your-oura-client-id"
callback_bind = "127.0.0.1:8788"
callback_path = "/callback"
requested_scopes = ["personal", "daily", "heartrate"]
```

Set the client secret in the environment instead of the config file:

```bash
export RINGMASTER_OURA_CLIENT_SECRET="your-oura-client-secret"
```

Important notes:

- granted scopes are no longer configured in `config.toml`; they come from the persisted auth session
- token secrets are intentionally kept out of plaintext config
- live auth and sync are local-first only; there is no required webhook infrastructure for the first usable release

## What is real today

- `auth login` is real for the phase-1 slice
- `sync once` is real for:
  - `personal_info`
  - `daily_sleep`
  - `daily_readiness`
  - `daily_activity`
  - `heartrate`
- demo mode is deterministic and still works without network or credentials
- fixture sync is real and is used for tests and CI smoke coverage

Still intentionally deferred:

- webhook delivery and subscription lifecycle
- scheduled/background polling
- broader Oura collections beyond the phase-1 slice
- release automation and packaging

## Architecture summary

The codebase stays in a single crate for now, with narrow module boundaries:

- `src/cli.rs`: CLI parsing and help text
- `src/config.rs`: config loading, XDG paths, runtime defaults
- `src/app.rs`: app state, screen models, demo/live data shaping
- `src/tui.rs`: Ratatui event loop and snapshot rendering
- `src/components/*`: pure rendering for Dashboard, Timeline, Trends, and Ops
- `src/store/*`: SQLite plan, migrations, typed store queries
- `src/oura/*`: OAuth loopback flow, token lifecycle ownership, typed client interface, sync orchestration

UI components do not perform network calls, token refresh, or database writes. The TUI reads presentation models only.

More detail lives in [docs/ARCHITECTURE.md](/home/ubuntu/ringmaster.rs/docs/ARCHITECTURE.md).

The storage backend choice is documented separately in [docs/decisions/20260408-storage-backend-rusqlite.md](/home/ubuntu/ringmaster.rs/docs/decisions/20260408-storage-backend-rusqlite.md).

## Verification

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo run -- doctor
cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase1
```

`cargo run -- demo` is also worth a quick smoke check, especially when changing layouts or snapshot flows.

## Next milestone

The next slice is to deepen the data model on top of the now-real auth/sync foundation: richer trends, additional Oura collections, and background polling that still respects the local-first UI boundaries.
