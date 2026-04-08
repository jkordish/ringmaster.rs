# ARCHITECTURE.md

## Scope

This document describes the implemented bootstrap architecture for `ringmaster.rs`. It reflects the real phase-0 / phase-1 foundation in the repository today, not the eventual end-state product.

## Design goals

- local-first by default
- poll-first for v1
- pure UI components
- single-crate simplicity until pressure justifies more structure
- deterministic demo mode for development, CI, and screenshots

## Runtime shape

```text
CLI
  -> config loading + tracing init
  -> runtime path setup
  -> command dispatcher

doctor / sync / auth
  -> store + Oura seams
  -> formatted text output

tui / demo
  -> app state builder
  -> Ratatui event loop
  -> pure screen renderers
```

## Module boundaries

### `src/cli.rs`

Responsibilities:

- `clap` parsing
- nested subcommand structure
- help text rendering

Non-responsibilities:

- config loading
- side effects
- command execution

### `src/config.rs`

Responsibilities:

- XDG-friendly path resolution
- config file parsing from `config.toml`
- environment overrides
- runtime directory creation
- Oura/logging defaults

Current defaults:

- config: `~/.config/ringmaster/config.toml`
- state: `~/.local/state/ringmaster`
- cache: `~/.cache/ringmaster`
- database: `ringmaster.db`
- OAuth callback: `http://127.0.0.1:8788/callback`

### `src/app.rs`

Responsibilities:

- screen enum and navigation state
- demo/live application models
- user-facing status/footer text
- shaping store/auth data into presentation structs

The app layer is where “local store snapshot + auth/capability diagnostics” become screen-specific models. It deliberately does not own terminal I/O, HTTP, or SQL.

### `src/tui.rs`

Responsibilities:

- interactive Ratatui event loop
- terminal session lifecycle
- keyboard-to-action mapping
- deterministic snapshot rendering via `TestBackend`

Why snapshot rendering exists:

- keeps demo mode useful without a TTY
- supports stable tests and CI smoke checks
- reuses the same component tree as the interactive UI

### `src/components/*`

Responsibilities:

- pure rendering for Dashboard, Timeline, Trends, and Ops

Boundary rule:

- components receive presentation models only
- no network calls
- no SQLite handles
- no token refresh logic

### `src/store/*`

Responsibilities:

- SQLite opening/configuration
- migration runner
- typed query surfaces
- sync-state persistence
- view-oriented read models

Current schema families:

- `app_metadata`
- `sync_state`
- `raw_payload_cache`
- `daily_sleep`
- `daily_readiness`
- `daily_activity`
- `heartrate_samples`
- `workouts`
- `tags`
- `enhanced_tags`
- `sessions`
- `webhook_subscriptions`

The schema intentionally includes webhook metadata now so later webhook work lands without reshaping unrelated storage.

### `src/oura/*`

Responsibilities:

- OAuth planning and callback router scaffold
- capability/scope modeling
- typed client interface
- poll-first sync orchestration scaffold

Current behavior:

- `auth login` prepares an authorization URL when credentials exist
- a loopback router scaffold exists for the configured callback path
- secure token persistence is intentionally deferred
- `sync once` records readiness/blocked/partial status in SQLite instead of pretending endpoint imports already exist

## Data flow

### Live TUI

```text
config
  -> Store::open()
  -> auth::inspect_auth()
  -> app::build_live_state()
  -> tui::run()
  -> components draw presentation models only
```

### Demo TUI

```text
config
  -> app::build_demo_state()
  -> tui::run() or tui::render_snapshot()
```

### Sync

```text
config
  -> auth::inspect_auth()
  -> ReqwestOuraClient capability surface
  -> sync::sync_once()
  -> store.sync_state().upsert(...)
```

## Why `rusqlite`

Bootstrap uses `rusqlite` instead of `sqlx` because:

- it keeps the dependency graph tighter
- the app is local-first and single-user
- migrations and typed query boundaries are straightforward at this stage
- async database orchestration is not required yet for the current command surface

We also considered Diesel as a possible path if PostgreSQL ever becomes a real product requirement, but we are deliberately not paying that abstraction cost during the SQLite-first bootstrap.

The detailed decision record lives in [docs/decisions/20260408-storage-backend-rusqlite.md](/home/ubuntu/ringmaster.rs/docs/decisions/20260408-storage-backend-rusqlite.md).

If sync/import throughput later justifies an async or pooled storage story, or if multi-backend support becomes real rather than hypothetical, that decision can be revisited with real pressure.

## Follow-up work

- complete callback capture + token exchange + token persistence
- implement real Oura API fetchers behind the typed client interface
- populate daily/trend/timeline views from imported data instead of empty-state scaffolding
- add scheduled/background polling
- add webhook subscription management once poll-first sync is stable
