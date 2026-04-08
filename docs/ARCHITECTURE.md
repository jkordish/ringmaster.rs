# ARCHITECTURE.md

## Scope

This document describes the implemented phase-2 MVP architecture for `ringmaster.rs`. It reflects the code that exists in the repository today, not the eventual end-state product.

## Design goals

- local-first by default
- poll-first for v1
- pure UI components
- single-crate simplicity until pressure justifies more structure
- deterministic demo mode for development, CI, and screenshots
- one real vertical slice before broadening the Oura surface
- explicit freshness semantics instead of vague "loading/error" buckets
- useful daily operation before expanding features

## Runtime shape

```text
CLI
  -> config loading + tracing init
  -> runtime path setup
  -> command dispatcher

doctor / auth / sync once
  -> store + auth/session seams
  -> typed Oura client boundaries
  -> formatted text output

sync watch
  -> refresh scheduler core
  -> family-selective sync engine
  -> bounded/demo watch mode for smoke tests

tui / tui --demo
  -> app state builder
  -> Event -> Action -> State -> Render loop
  -> background refresh worker
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
- refresh policy defaults
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
- explicit freshness and availability modeling
- demo/live application snapshots and presentation models
- user-facing status/footer text
- shaping store/auth data into presentation structs
- deterministic insight summaries

The app layer is where persisted store rows and auth/capability diagnostics become screen-specific models. It deliberately does not own terminal I/O, HTTP, or SQL.

Important implemented state concepts:

- `FreshnessKind`: `Fresh`, `Stale`, `NoDataYet`, `NeverSynced`, `MissingScope`, `AuthFailure`, `SourceDelayed`
- `LiveSnapshot`: the immutable persisted-data snapshot sent into the reducer after each background refresh
- `TrendWindowKind`: 7d / 30d / 90d user-facing trend windows

### `src/tui.rs`

Responsibilities:

- interactive Ratatui event loop
- terminal session lifecycle
- keyboard-to-action mapping
- live background refresh worker wiring
- deterministic snapshot rendering via `TestBackend`

Why snapshot rendering exists:

- keeps demo mode useful without a TTY
- supports stable tests and CI smoke checks
- reuses the same component tree as the interactive UI

How background refresh works:

- the main loop stays focused on terminal input, tick events, reducer updates, and rendering
- a dedicated worker thread owns a single-thread Tokio runtime
- that worker opens the store on its own thread, uses the scheduler core from `src/refresh.rs`, and calls the same `sync_selected(...)` path as `sync once`
- when new persisted data is available, the worker rebuilds a `LiveSnapshot` and sends `Action::LiveSnapshotLoaded` back to the UI loop
- this keeps blocking store/auth/sync work off the render path and avoids `Send` pressure on the SQLite + sync stack

### `src/components/*`

Responsibilities:

- pure rendering for Dashboard, Timeline, Trends, and Ops

Boundary rule:

- components receive presentation models only
- no network calls
- no SQLite handles
- no token refresh logic

The components are intentionally presentation-only:

- Dashboard renders cards, freshness/capability lists, and "what changed"
- Timeline renders the day selector, gap-aware heartrate chart, and selected-point details
- Trends renders window tabs, metric sparklines, and notes
- Ops renders trust/freshness metadata without reaching back into the store

### `src/refresh.rs`

Responsibilities:

- family-aware scheduler decisions
- interval policy
- persisted backoff handling
- bounded watch execution for demo/CI

The scheduler is intentionally reusable by both `sync watch` and the live TUI worker. Webhook invalidation remains deferred, but the current shape leaves an obvious seam for future external triggers.

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

`sync_state` tracks per-slice status, watermark, granted scopes, failure counts, next-attempt backoff, and the last structured Oura problem. `raw_payload_cache` is intentionally separate from normalized tables so future debugging and replay work does not leak SQL concerns into the transport layer.

Read-side queries now expose enough shape for the MVP screens:

- latest personal profile snapshot
- rolling daily history for baseline/trend calculations
- heartrate-by-day queries for the timeline
- available heartrate day selection lists
- sync/auth diagnostics for the Ops screen

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
- `sync once` and `sync watch` import the current MVP slice:
  - `/v2/usercollection/personal_info`
  - `/v2/usercollection/daily_sleep`
  - `/v2/usercollection/daily_readiness`
  - `/v2/usercollection/daily_activity`
  - `/v2/usercollection/heartrate`
- sync remains family-selective internally, so the scheduler can refresh only the families that are due without inventing a separate import path

### `src/insights.rs`

Responsibilities:

- 7d and 30d baselines
- day-over-day deltas
- deviation scoring when history is sufficient
- confidence notes when the history is too thin

This module is intentionally small and deterministic. It does not make causal claims or try to behave like a medical interpretation layer.

## Data flow

### Live TUI

```text
config
  -> Store::open()
  -> auth::inspect_auth()
  -> app::build_live_state()
  -> tui::run()
  -> worker thread schedules refreshes
  -> worker runs sync_selected(...)
  -> worker rebuilds LiveSnapshot
  -> Action::LiveSnapshotLoaded enters reducer
  -> components draw presentation models only
```

The TUI never performs HTTP, token refresh, or database writes on the render path. Live screens render only from persisted auth/session metadata and SQLite read models.

### Demo TUI

```text
config
  -> app::build_demo_state()
  -> tui::run() or tui::render_snapshot()
```

### `sync once`

```text
config
  -> auth::ensure_authorized_session()
  -> ReqwestOuraClient
  -> sync::sync_once()
  -> raw payload cache + normalized upserts
  -> store.sync_state().upsert(...)
```

### `sync watch`

```text
config
  -> refresh::due_families()/next_wake_duration()
  -> sync::sync_selected()
  -> store.sync_state().upsert(...)
  -> optional bounded exit for demo/CI
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

## Freshness semantics

Each family is evaluated independently and surfaced explicitly in the UI:

- `fresh`: data is inside its configured freshness window
- `stale`: persisted data exists, but it is too old or the last refresh was partial
- `no data yet`: sync ran but there are still no rows for the family
- `never synced`: the family has not completed a sync
- `missing scope`: the required Oura scope is not granted
- `auth failure`: persisted sync state points to auth/session failure
- `source delayed`: Oura has not closed out the daily family yet, so the app compares against the latest fully available day

## Follow-up work

- webhook invalidation feeding the existing scheduler
- deeper daily and heartrate derived views on top of the current MVP
- broader Oura collections beyond personal/daily/heartrate
- packaging and release automation
