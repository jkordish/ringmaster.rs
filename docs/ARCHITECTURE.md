# ARCHITECTURE.md

## Scope

This document describes the implemented phase-3 architecture for `ringmaster.rs`. It reflects the code that exists in the repository today, not the eventual end-state product.

## Design goals

- local-first by default
- poll-first for v1
- pure UI components
- single-crate simplicity until pressure justifies more structure
- deterministic demo and fixture paths for development, CI, and screenshots
- explicit freshness and availability semantics instead of vague loading/error buckets
- explainability that is evidence-based and restrained
- deterministic derived analytics rather than pseudo-intelligence

## Runtime shape

```text
CLI
  -> config loading + tracing init
  -> runtime path setup
  -> command dispatcher

doctor / auth / sync once / sync watch / derive rebuild
  -> store + auth/session seams
  -> typed Oura client boundaries
  -> normalized imports
  -> derived rebuilds
  -> formatted text output

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

Current refresh defaults now cover all six live families:

- personal
- daily
- heartrate
- workouts
- enhanced tags
- sessions

History and overlap defaults are also family-aware, so the sync layer does not need to invent bespoke window logic in the UI or import path.

### `src/app.rs`

Responsibilities:

- screen enum and navigation state
- explicit freshness and availability modeling
- demo/live application snapshots and presentation models
- shared selected-day and selected-event state
- user-facing status/footer text
- shaping store/auth/derived data into screen-specific models
- deterministic selected-day summaries and pattern rows

The app layer is where persisted normalized rows, derived tables, and auth/capability diagnostics become screen models. It deliberately does not own terminal I/O, HTTP, or SQL.

Important implemented state concepts:

- `FreshnessKind`: `Fresh`, `Stale`, `NoDataYet`, `NeverSynced`, `MissingScope`, `AuthFailure`, `SourceDelayed`
- `LiveSnapshot`: the immutable persisted-data snapshot sent into the reducer after each background refresh
- `selected_day_index`: shared by Dashboard, Timeline, and Explain
- `selected_event_id`: shared by Timeline and Explain
- `overlay_filters`: shared family toggles for workouts, tags, and sessions
- `PatternMetricFilter`: shared pattern metric filtering for the Patterns screen

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

Implemented screen set:

- Dashboard
- Timeline
- Trends
- Explain
- Patterns
- Ops

### `src/components/*`

Responsibilities:

- pure rendering for Dashboard, Timeline, Trends, Explain, Patterns, and Ops

Boundary rule:

- components receive presentation models only
- no network calls
- no SQLite handles
- no token refresh logic

Component responsibilities today:

- Dashboard renders selected-day summaries, cards, freshness/capability lists, and “what likely changed?”
- Timeline renders the gap-aware heartrate chart, overlay lanes, selected-event details, and selected-day event list
- Trends renders window tabs, sparklines, and trend notes
- Explain renders selected-day summary lines, measurements, evidence, context entries, and caveats
- Patterns renders deterministic association rows plus sufficiency/wording notes
- Ops renders trust and freshness diagnostics without reaching back into the store

### `src/refresh.rs`

Responsibilities:

- family-aware scheduler decisions
- interval policy
- persisted backoff handling
- bounded watch execution for demo/CI

The scheduler is reusable by both `sync watch` and the live TUI worker. It now treats `workout`, `enhanced_tag`, and `session` as first-class sync families with distinct intervals, stale-after windows, and sync keys.

### `src/store/*`

Responsibilities:

- SQLite opening/configuration
- migration runner
- typed query surfaces
- sync-state persistence
- view-oriented read models
- derived table writes and reads

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
- `derived_context_events`
- `derived_pattern_summaries`

Important query responsibilities added in phase 3:

- normalized reads for workouts, enhanced tags, and sessions
- selected-day and day-range context-event queries
- persisted pattern summary reads
- record counts that include both normalized and derived tables

`sync_state` still tracks per-slice status, watermark, granted scopes, failure counts, next-attempt backoff, and the last structured Oura problem. `raw_payload_cache` remains intentionally separate from normalized tables so replay and debugging do not leak transport details into the UI.

### `src/oura/*`

Responsibilities:

- loopback OAuth login
- token refresh lifecycle ownership
- capability/scope modeling
- typed transport DTOs and client boundary
- poll-first sync orchestration

Current live sync behavior:

- `auth login` prints an authorization URL, listens on the configured loopback callback, validates CSRF state, exchanges the code server-side, and persists auth/session metadata
- token secrets live behind the keyring-backed `SecretStore` seam; tests use an in-memory secret store
- `ensure_authorized_session` is the single owner for access-token refresh
- `ReqwestOuraClient` and `FixtureOuraClient` share the same typed fetch surface
- `sync once` and `sync watch` import:
  - `/v2/usercollection/personal_info`
  - `/v2/usercollection/daily_sleep`
  - `/v2/usercollection/daily_readiness`
  - `/v2/usercollection/daily_activity`
  - `/v2/usercollection/heartrate`
  - workouts
  - enhanced tags
  - sessions

Each family is imported through idempotent upserts and family-specific reconcile windows. Missing scopes are captured explicitly so the product can show “missing capability” rather than pretending the family is simply empty.

### `src/derive.rs`

Responsibilities:

- deterministic rebuild of canonical context events
- deterministic rebuild of persisted pattern summaries
- fixture-backed demo rebuild path
- one place for derivation logic shared by CLI rebuilds and product reads

`derive rebuild` follows this shape:

```text
open store
  -> read normalized workouts / tags / enhanced tags / sessions
  -> build canonical context events
  -> build descriptive pattern summaries from daily history + context events
  -> replace derived tables atomically through typed store APIs
```

The rebuild path is safe to run repeatedly and intentionally avoids any UI or live-network coupling.

### `src/insights.rs`

Responsibilities:

- rolling baselines
- day-over-day deltas
- deviation scoring when history is sufficient
- confidence notes when history is too thin

This module remains small and deterministic. Explain and Patterns may use its baseline semantics, but they do not turn into a freeform narrative or causal interpretation layer.

## Canonical context-event model

`derived_context_events` is the canonical read model for Timeline and Explain.

It unifies workouts, legacy tags, enhanced tags, and sessions behind normalized fields:

- stable derived id
- family
- source id
- anchor day
- start / end timestamps
- time semantics (`interval`, `point`, `all_day`)
- title
- subtype
- notes
- intensity
- metadata JSON for drill-down
- updated timestamp

Why it exists:

- avoids assembling overlays ad hoc inside widgets
- gives Explain a stable evidence source
- keeps overlap handling deterministic
- provides one extension seam for future families

## Explainability and pattern architecture

### Explain

Explain is not a language-model feature. It is a deterministic presentation layer over:

- selected-day daily metrics
- rolling baselines
- nearby derived context events
- capability/freshness diagnostics

Explain deliberately uses disciplined templates and caveat rules:

- no medical advice
- no causal claims
- no significance theater
- no “AI says” framing

### Patterns

Patterns is a deterministic descriptive-association layer over persisted history.

Current implementation:

- normalizes event keys by family
- computes event-occurrence metric deltas against rolling baselines
- aggregates with simple, documented summaries
- requires a minimum sample threshold before surfacing rows
- persists the resulting summary rows for cheap UI reads

Current wording rules:

- `associated with`
- `co-occurred with`
- `after days with`
- never `caused by`

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
  -> worker rebuilds LiveSnapshot from persisted + derived tables
  -> Action::LiveSnapshotLoaded enters reducer
  -> components draw presentation models only
```

The TUI never performs HTTP, token refresh, or database writes on the render path. Live screens render only from persisted auth/session metadata, normalized SQLite rows, and derived SQLite rows.

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
  -> ReqwestOuraClient or FixtureOuraClient
  -> sync::sync_once()
  -> raw payload cache + normalized upserts
  -> store.sync_state().upsert(...)
```

### `derive rebuild`

```text
config
  -> Store::open()
  -> derive::rebuild()
  -> replace_context_events(...)
  -> replace_pattern_summaries(...)
```

In `--demo` mode, the rebuild command first seeds a temporary store from the phase-3 fixtures using the same sync engine and then rebuilds the derived tables from that persisted data.

## Boundary checks

The current architecture intentionally preserves the following constraints:

- UI components do not know about HTTP or SQLite
- the auth layer remains the sole refresh-token owner
- sync code writes normalized and raw data, but does not know about Ratatui widgets
- derivation happens from persisted data, not by ad hoc widget joins
- background work stays off the render path

## Intentionally deferred

- webhook delivery and subscription lifecycle
- broader Oura endpoints beyond the current phase-3 surface
- export/share/report workflows
- generalized machine learning
- cloud sync services or multi-user architecture
