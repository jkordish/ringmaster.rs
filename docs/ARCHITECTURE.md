# ARCHITECTURE.md

## Scope

This document describes the implemented phase-1 architecture for `ringmaster.rs`. It reflects the code that exists in the repository today, not the eventual end-state product.

## Design goals

- local-first by default
- poll-first for v1
- pure UI components
- single-crate simplicity until pressure justifies more structure
- deterministic demo mode for development, CI, and screenshots
- one real vertical slice before broadening the Oura surface

## Runtime shape

```text
CLI
  -> config loading + tracing init
  -> runtime path setup
  -> command dispatcher

doctor / sync / auth
  -> store + auth/session seams
  -> typed Oura client boundaries
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
- env-only client secret handling

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

The app layer is where persisted store rows and auth/capability diagnostics become screen-specific models. It deliberately does not own terminal I/O, HTTP, or SQL.

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
- `auth_session`
- `sync_state`
- `raw_payload_cache`
- `personal_info`
- `daily_sleep`
- `daily_readiness`
- `daily_activity`
- `heartrate_samples`
- `workouts`
- `tags`
- `enhanced_tags`
- `sessions`
- `webhook_subscriptions`

`sync_state` tracks per-slice status, watermark, granted scopes, and the last structured Oura problem. `raw_payload_cache` is intentionally separate from normalized tables so future debugging and replay work does not leak SQL concerns into the transport layer.

### `src/oura/*`

Responsibilities:

- loopback OAuth login
- token refresh lifecycle ownership
- capability/scope modeling
- typed transport DTOs and client boundary
- poll-first sync orchestration

Current behavior:

- `auth login` prints an authorization URL, listens on the configured loopback callback, validates CSRF state, exchanges the code server-side, and persists auth/session metadata
- token secrets live behind the keyring-backed `SecretStore` seam; tests use an in-memory secret store
- `ensure_authorized_session` is the single owner for access-token refresh
- `ReqwestOuraClient` and `FixtureOuraClient` share the same typed phase-1 fetch surface
- `sync once` imports the current phase-1 slice:
  - `/v2/usercollection/personal_info`
  - `/v2/usercollection/daily_sleep`
  - `/v2/usercollection/daily_readiness`
  - `/v2/usercollection/daily_activity`
  - `/v2/usercollection/heartrate`

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

The TUI never performs HTTP, token refresh, or database writes. Live screens render only from persisted auth/session metadata and SQLite read models.

### Demo TUI

```text
config
  -> app::build_demo_state()
  -> tui::run() or tui::render_snapshot()
```

### Live sync

```text
config
  -> auth::ensure_authorized_session()
  -> ReqwestOuraClient
  -> sync::sync_once()
  -> raw payload cache + normalized upserts
  -> store.sync_state().upsert(...)
```

### Fixture sync

```text
config
  -> FixtureOuraClient
  -> sync::sync_once(dry_run or fixture mode)
  -> same normalization logic as live sync
  -> optional no-write smoke path for CI
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

- deepen trend calculations on top of the now-real daily/heartrate slice
- expand the Oura surface beyond personal/daily/heartrate
- add scheduled/background polling
- add webhook subscription management once poll-first sync is stable
