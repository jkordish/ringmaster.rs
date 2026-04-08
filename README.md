# ringmaster.rs

`ringmaster.rs` is a local-first Rust terminal application for exploring Oura Cloud data with a Ratatui interface, SQLite-backed local storage, deterministic demo mode, and a poll-first Oura Cloud API v2 integration.

## Status

This repository now includes a daily-drivable MVP:

- `clap` CLI with `tui`, `tui --demo`, `doctor`, `auth login`, `sync once`, `sync watch`, and the compatibility alias `demo`
- a useful Ratatui Dashboard, Timeline, Trends, and Ops screen backed by persisted SQLite data
- deterministic demo data that exercises the same screen tree without credentials or network access
- real loopback OAuth login with server-side code exchange, PKCE, and CSRF-safe state handling
- persisted auth/session metadata in SQLite with token secrets stored through the OS keyring seam
- real poll-first sync for personal info, daily summaries, and heartrate into normalized tables plus raw payload cache
- family-aware background refresh while the TUI is open, plus the same scheduler exposed as `sync watch`
- explicit freshness and availability semantics for fresh, stale, missing scope, no data yet, never synced, auth failure, and source-delayed data
- a small deterministic insight layer with 7d/30d baselines, day-over-day deltas, and confidence notes
- structured logging via `tracing`

The project is intentionally not feature-complete yet. The goal is a trustworthy local foundation with one real vertical slice, not a one-shot full product dump.

## Commands

```bash
cargo run -- tui
cargo run -- tui --demo
cargo run -- doctor
cargo run -- auth login
cargo run -- sync once
cargo run -- sync watch
cargo run -- sync watch --demo --max-iterations 1
cargo run -- demo
```

Rust toolchain baseline: `rust-version = 1.88`.

Behavior notes:

- `ringmaster tui` launches the live TUI when attached to a terminal. Without a TTY it renders a snapshot of the current local app state instead.
- `ringmaster tui --demo` launches the same UI shell in deterministic demo mode. Without a TTY it renders a text snapshot, which is useful for CI and screenshot-oriented workflows.
- `ringmaster demo` remains as a compatibility alias for `ringmaster tui --demo`.
- `ringmaster doctor` resolves paths, initializes SQLite, applies migrations, and prints auth, capability, per-family freshness, refresh policy, and path diagnostics.
- `ringmaster auth login` starts a loopback OAuth flow, validates state, exchanges the code server-side, and persists auth/session metadata locally.
- `ringmaster sync once` refreshes auth when needed, imports the MVP Oura slice, caches raw payloads, and upserts normalized SQLite rows.
- `ringmaster sync watch` runs the same family-aware scheduler used by the live TUI, but without the UI. This is the debugging and automation-friendly watch path.
- `ringmaster sync watch --demo --max-iterations 1` is the bounded smoke path for CI and local verification. It uses the checked-in fixtures by default and exits after one scheduler iteration.
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

[refresh]
personal_interval_secs = 3600
daily_interval_secs = 300
heartrate_interval_secs = 60
personal_stale_after_secs = 259200
daily_stale_after_secs = 43200
heartrate_stale_after_secs = 900
daily_history_days = 90
daily_overlap_days = 2
heartrate_history_days = 7
heartrate_overlap_minutes = 60
max_backoff_secs = 3600
```

Set the client secret in the environment instead of the config file:

```bash
export RINGMASTER_OURA_CLIENT_SECRET="your-oura-client-secret"
```

Important notes:

- granted scopes are no longer configured in `config.toml`; they come from the persisted auth session
- token secrets are intentionally kept out of plaintext config
- live auth and sync are local-first only; there is no required webhook infrastructure for the first usable release

## MVP behavior

What the screens now do:

- Dashboard shows today's sleep, readiness, and activity cards from persisted daily rows, freshness badges, capability badges, a compact baseline summary, and a "what changed" panel driven by the deterministic insight layer.
- Timeline shows an intraday heartrate chart for a selected cached day, handles gaps in the sample stream, and exposes selected-point details plus a source legend.
- Trends shows 7d / 30d / 90d windows, baseline-aware trend summaries, sparklines for daily metrics, and confidence notes when history is thin.
- Ops makes trust explicit with auth state, granted scopes, token metadata, last sync per family, current database/config paths, and the active refresh policy.

While `ringmaster tui` is open, a background refresh worker reuses the same sync engine as `sync once` and `sync watch`. The render path stays pure: widgets read presentation models only, while the refresh worker opens the store on its own thread and feeds snapshot updates back into the main event loop as actions.

## Freshness semantics

The UI does not collapse every problem into a generic error. Each data family resolves to a specific state:

- `fresh`: the family has recent persisted data inside its freshness window
- `stale`: data exists, but it is older than the configured freshness policy or the last refresh was partial
- `no data yet`: sync ran, but there are still no persisted rows for that family
- `never synced`: no successful sync has happened yet
- `missing scope`: the required Oura scope was not granted
- `auth failure`: the last failure was due to auth/session problems
- `source delayed`: Oura has not closed out the latest daily family yet, so the app compares against the latest fully available day instead of pretending today's row exists

Default responsive refresh policy:

- personal: every 3600s, stale after 72h
- daily: every 300s, stale after 12h
- heartrate: every 60s, stale after 15m

## What is real today

- `auth login` is real for the MVP slice
- `sync once` is real for:
  - `personal_info`
  - `daily_sleep`
  - `daily_readiness`
  - `daily_activity`
  - `heartrate`
- `sync watch` and the live TUI reuse the same scheduler and family-selective sync engine
- demo mode is deterministic and still works without network or credentials
- fixture sync is real and is used for tests and CI smoke coverage

Still intentionally deferred:

- webhook delivery and subscription lifecycle
- broader Oura collections beyond the phase-1 slice
- release automation and packaging

## Architecture summary

The codebase stays in a single crate for now, with narrow module boundaries:

- `src/cli.rs`: CLI parsing and help text
- `src/config.rs`: config loading, XDG paths, runtime defaults
- `src/app.rs`: app state, explicit freshness modeling, screen models, demo/live data shaping
- `src/tui.rs`: Ratatui event loop, background refresh worker wiring, and snapshot rendering
- `src/components/*`: pure rendering for Dashboard, Timeline, Trends, and Ops
- `src/store/*`: SQLite plan, migrations, typed store queries, and sync-state persistence
- `src/oura/*`: OAuth loopback flow, token lifecycle ownership, typed client interface, and sync orchestration
- `src/refresh.rs`: reusable scheduler core for the TUI and `sync watch`
- `src/insights.rs`: deterministic derived-data helpers for baselines, deltas, and confidence notes

UI components do not perform network calls, token refresh, or database writes. The TUI reads presentation models only.

More detail lives in [docs/ARCHITECTURE.md](/home/ubuntu/ringmaster.rs/docs/ARCHITECTURE.md).

The storage backend choice is documented separately in [docs/decisions/20260408-storage-backend-rusqlite.md](/home/ubuntu/ringmaster.rs/docs/decisions/20260408-storage-backend-rusqlite.md).

## Verification

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo run -- doctor
cargo run -- sync watch --demo --max-iterations 1
```

`cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase1` is still useful when changing importer behavior, and `cargo run -- tui --demo` remains a handy non-network layout smoke path.
