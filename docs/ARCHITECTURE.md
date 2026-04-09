# ARCHITECTURE.md

## Scope

This document describes the implemented phase-7 architecture for `ringmaster.rs`. It reflects the code that exists in the repository today, not the eventual end-state product.

## Design goals

- local-first by default
- pure UI components
- single-crate simplicity until pressure justifies more structure
- deterministic demo, fixture, and replay paths for development and CI
- explicit freshness and availability semantics instead of vague loading/error buckets
- webhook-first freshness where Oura supports it
- honest scheduled fallback where Oura does not
- auditable operations when freshness goes wrong
- deterministic derived analytics rather than pseudo-intelligence
- bounded smart reviews and investigations rather than a chat assistant

## Runtime shape

```text
CLI
  -> config loading + tracing init
  -> runtime path setup
  -> command dispatcher

doctor / auth / sync once / sync watch / derive rebuild / review today / review week / review investigate
  -> store + auth/session seams
  -> typed Oura client boundaries
  -> normalized imports
  -> bounded auto-derived rebuilds after sync, plus explicit full-history rebuilds
  -> deterministic review feature snapshots and ranking
  -> formatted text output

webhook serve
  -> axum receiver
  -> verification challenge + signature verification
  -> accepted/rejected delivery audit
  -> invalidation enqueue
  -> heartbeat updates

webhook subscriptions list / sync
  -> desired subscription config
  -> app-credential admin client or fixture-backed remote state
  -> explicit diff/reporting
  -> remote snapshot persistence

webhook replay
  -> fixture or stored-delivery envelope loading
  -> same receive/verify/enqueue path
  -> bounded invalidation-processing preview for fixtures, re-enqueue only for stored deliveries

tui / tui --demo
  -> app state builder
  -> Event -> Action -> State -> Render loop
  -> background refresh worker
  -> pure screen renderers

ui snapshot
  -> app state builder
  -> deterministic screen + size matrix selection
  -> optional phase-7 scenario matrix expansion
  -> shared Ratatui render path
  -> text artifact writing for visual QA
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

The CLI now exposes a distinct `webhook` command family instead of collapsing receiver, replay, and watch behavior into one giant process. It also exposes `ui snapshot` as a dedicated non-interactive design-QA path instead of relying on ad hoc `tui --demo` capture.

### `src/config.rs`

Responsibilities:

- XDG-friendly path resolution
- config file parsing from `config.toml`
- environment overrides
- runtime directory creation
- Oura/logging defaults
- refresh policy defaults
- env-only client secret handling
- webhook receiver and subscription defaults

Current refresh defaults cover all six live families:

- personal
- daily
- heartrate
- workouts
- enhanced tags
- sessions

Current webhook config covers:

- receiver bind/path
- public callback URL metadata
- verification token
- signature timestamp tolerance
- heartbeat cadence
- renewal lead window
- desired subscription specs

### `src/app.rs`

Responsibilities:

- screen enum and navigation state
- explicit freshness and availability modeling
- demo/live application snapshots and presentation models
- shared selected-day and selected-event state
- shared selected-review state
- user-facing status/footer text
- shaping store/auth/derived/webhook data into screen-specific models
- deterministic selected-day summaries and pattern rows
- deterministic review decks and bounded investigations

The app layer is where persisted normalized rows, derived tables, auth diagnostics, sync provenance, subscription state, delivery history, queue state, and runtime heartbeats become screen models. It deliberately does not own terminal I/O, HTTP, or SQL.

Important implemented state concepts:

- `FreshnessKind`:
  - `FreshWebhook`
  - `FreshPeriodic`
  - `StaleNoRecentDelivery`
  - `StaleSyncFailed`
  - `StaleUnsupportedWebhook`
  - `StaleReceiverDown`
  - `StaleSubscriptionMissing`
  - `StaleCapabilityMissing`
  - `StaleUpstreamPending`
- `LiveSnapshot`: the immutable persisted-data snapshot sent into the reducer after each background refresh
- `WebhookOpsSnapshot`: persisted receiver/subscription/delivery/queue/runtime view data shaped for Ops and `doctor`
- `selected_day_index`: shared by Dashboard, Timeline, Explain, and Review
- selected-day continuity preserves the exact selected day when possible, then the nearest earlier available day, then the next later day, before falling back to the newest day
- `selected_event_id`: shared by Timeline and Explain
- `overlay_filters`: shared family toggles for workouts, tags, and sessions
- `PatternMetricFilter`: shared pattern metric filtering for the Patterns screen
- `review_mode`: Today, Week, or Investigate within the Review screen
- `review_focus`: readiness, sleep, recovery, stress, or activity within Investigate mode
- `selected_review_card_index`: selected ranked card within Review

### `src/tui.rs`

Responsibilities:

- interactive Ratatui event loop
- terminal session lifecycle
- keyboard-to-action mapping
- live background refresh worker wiring
- deterministic snapshot rendering via `TestBackend`
- shared frame chrome driven by semantic theme and viewport context

Why snapshot rendering exists:

- keeps demo mode useful without a TTY
- supports stable tests and CI smoke checks
- reuses the same component tree as the interactive UI
- gives the repo a canonical scenario-matrix QA path without duplicating widget logic

The shared frame now owns the design-system-driven shell:

- semantic header and active-screen treatment
- consistent footer/help strip
- centralized compact/medium/wide viewport context
- shared panel/badge/divider language used by all screens

How background refresh works:

- the main loop stays focused on terminal input, tick events, reducer updates, and rendering
- a dedicated worker thread owns a single-thread Tokio runtime
- that worker opens the store on its own thread, uses the shared scheduler core from `src/refresh.rs`, and calls the same `sync_selected(...)` path as `sync once`
- when new persisted data is available, the worker rebuilds a `LiveSnapshot` and sends `Action::LiveSnapshotLoaded` back to the UI loop
- this keeps blocking store/auth/sync work off the render path and avoids `Send` pressure on the SQLite + sync stack

Implemented screen set:

- Dashboard
- Timeline
- Trends
- Explain
- Patterns
- Ops
- Review

### `src/ui/*`

Responsibilities:

- semantic palette and emphasis roles
- breakpoint-aware spacing and layout helpers
- reusable chrome/panel/badge builders
- shared chart grammar
- deterministic multi-screen snapshot artifact generation
- phase-7 scenario tagging for `strong`, `weak`, `empty`, `stale`, and `error`

Implemented modules:

- `src/ui/theme.rs`: semantic palette roles, tones, and emphasis helpers
- `src/ui/layout.rs`: viewport classes and layout helpers
- `src/ui/chrome.rs`: section titles, panels, badges, and focus/state affordances
- `src/ui/charts.rs`: shared line/bar/spark styling and annotation helpers
- `src/ui/snapshot.rs`: deterministic `ui snapshot` artifact writing

The snapshot writer now supports two naming modes:

- legacy/demo mode: `screen-size.txt`
- phase-7 scenario mode: `screen-scenario-size.txt`

Boundary rule:

- these modules are presentation-only and do not own persistence, auth, sync, or network work

### `src/components/*`

Responsibilities:

- pure rendering for Dashboard, Timeline, Trends, Explain, Patterns, Ops, and Review
- screen-specific choreography using shared semantic theme, layout, and chrome helpers

Boundary rule:

- components receive presentation models only
- no network calls
- no SQLite handles
- no token refresh logic

Component responsibilities today:

- Dashboard renders the editorial front page: “what matters now,” the daily metric band, freshness/capability framing, and drill-down cues
- Timeline renders the chart-first temporal composition, overlay lanes, selected detail, and selected-day event list
- Trends renders the comparative scanning matrix with windows, deltas, spark hints, and baseline readouts
- Explain renders a deliberate evidence flow: claim, measured inputs, supporting evidence, context, and uncertainty
- Timeline, Explain, and Review include lightweight breadcrumbs only when they keep shared day or event context visible
- Patterns renders grouped associations and interpretive notes distinct from Explain
- Ops renders the utilitarian operator console with summary, family status, diagnostics, and warnings without reaching back into the store
- Review renders ranked briefing cards plus bounded investigation detail without making network or database calls

### `src/refresh.rs`

Responsibilities:

- family-aware scheduler decisions
- interval policy
- persisted backoff handling
- invalidation claim/process/settle loop
- bounded watch execution for demo and CI

The watch engine is reusable by both `sync watch` and the live TUI worker. It now treats:

- queued webhook invalidations as first-class work
- `workout`, `enhanced_tag`, and `session` delete events as explicit local delete side effects
- `heartrate` as scheduled-only fallback

`sync watch` remains the only long-running invalidation consumer. This preserves the operational split between “receive quickly” and “process carefully.”

### `src/store/*`

Responsibilities:

- SQLite opening and configuration
- migration runner
- typed query surfaces
- sync-state persistence
- view-oriented read models
- derived table writes and reads
- webhook audit, queue, and runtime metadata

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
- `sleep_time`
- `daily_stress`
- `daily_resilience`
- `daily_cardiovascular_age`
- `vo2_max`
- `rest_mode_periods`
- `webhook_desired_subscriptions`
- `webhook_remote_subscriptions`
- `webhook_deliveries`
- `webhook_delivery_rejections`
- `webhook_invalidations`
- `webhook_processing_attempts`
- `webhook_runtime_heartbeats`
- `derived_context_events`
- `derived_pattern_summaries`
- `derived_review_signal_days`

Important query responsibilities added in phase 5:

- normalized upserts and views for the six review-support Oura families
- persisted review signal snapshots rebuilt from local data
- typed reads for ranked review and investigation inputs

### `src/review/*`

Responsibilities:

- canonical review signal registry
- per-signal feature shaping into rebuildable daily snapshots
- deterministic ranking for today and week review decks
- bounded investigation assembly for fixed focuses
- deterministic user-facing templates shared by CLI and TUI

This layer is intentionally structured data first. It does not own open-ended prompting, hosted AI integration, or freeform chat semantics.

Important query responsibilities added in phase 4:

- desired and remote subscription persistence
- accepted and rejected delivery audit
- invalidation enqueue, claim, retry, and completion tracking
- receiver and watch heartbeat persistence
- sync trigger provenance persistence
- richer Ops-oriented read surfaces for subscriptions, deliveries, queue depth, and incidents

`sync_state` now tracks per-slice status, watermark, granted scopes, failure counts, next-attempt backoff, last structured Oura problem, and the last trigger source and trigger detail. `raw_payload_cache` remains intentionally separate from normalized tables so replay and debugging do not leak transport details into the UI.

### `src/store/webhook_store.rs`

Responsibilities:

- typed storage boundary for all webhook-specific persistence
- idempotent accepted-delivery writes
- rejected-delivery audit writes
- invalidation queue coalescing and lifecycle management
- remote subscription snapshot persistence
- runtime heartbeat writes and reads

The explicit split between raw delivery audit and derived invalidation queue is intentional: operators need to know both what arrived and what work it turned into.

### `src/oura/*`

Responsibilities:

- loopback OAuth login
- token refresh lifecycle ownership
- capability and scope modeling
- typed transport DTOs and client boundaries
- poll-first sync orchestration
- webhook admin API integration for subscription lifecycle

Current live sync behavior:

- `auth login` prints an authorization URL, listens on the configured loopback callback, validates CSRF state, exchanges the code server-side, and persists auth/session metadata
- token secrets live behind the keyring-backed `SecretStore` seam; tests use an in-memory secret store
- `ensure_authorized_session` is the single owner for access-token refresh
- `ReqwestOuraClient` and `FixtureOuraClient` share the same typed fetch surface
- the webhook admin client uses app credentials for subscription list/create/update/renew/delete flows
- `sync once` and `sync watch` import:
  - `/v2/usercollection/personal_info`
  - `/v2/usercollection/daily_sleep`
  - `/v2/usercollection/daily_readiness`
  - `/v2/usercollection/daily_activity`
  - `/v2/usercollection/heartrate`
  - `/v2/usercollection/daily_stress`
  - `/v2/usercollection/daily_resilience`
  - `/v2/usercollection/sleep_time`
  - `/v2/usercollection/daily_cardiovascular_age`
  - `/v2/usercollection/vO2_max`
  - `/v2/usercollection/rest_mode_period`
  - workouts
  - enhanced tags
  - sessions

Each family is imported through idempotent upserts and family-specific reconcile windows. Missing scopes are captured explicitly so the product can show “missing capability” rather than pretending the family is simply empty.

Successful non-dry-run syncs also refresh the derived context-event, pattern-summary, and review-signal tables over a bounded recent window, so Explain, Timeline overlays, Patterns, and Review stay current without making every background refresh reprocess the entire database. `derive rebuild` remains the explicit full-history recompute path.

### `src/webhook/*`

Responsibilities:

- receiver routing
- verification challenge handling
- signature and timestamp verification
- accepted and rejected delivery normalization
- replay plumbing
- declarative subscription diffing and execution

The webhook module is intentionally separate from the sync runtime. It owns ingress, auditing, and subscription management, while `src/refresh.rs` owns queue consumption and reconciliation.

### `src/derive.rs`

Responsibilities:

- deterministic rebuild of canonical context events
- deterministic rebuild of persisted pattern summaries
- deterministic rebuild of persisted review signal snapshots
- fixture-backed demo rebuild path
- one place for derivation logic shared by CLI rebuilds and product reads

`derive rebuild` still follows this shape:

```text
open store
  -> read normalized daily metrics, review-support families, workouts, tags, enhanced tags, and sessions
  -> build canonical context events
  -> build descriptive pattern summaries from daily history + context events
  -> build review signal snapshots from persisted local data
  -> replace derived tables atomically through typed store APIs
```

The rebuild path is safe to run repeatedly and intentionally avoids any UI or live-network coupling.

## Runtime modes

The product now has explicit operational modes:

- scheduler-only: no healthy receiver or no viable subscription state, but periodic fallback still runs
- receiver-only: receiver is healthy but watch is not actively processing queued invalidations
- hybrid: receiver and watch are both healthy, subscriptions are present, and the app can report webhook-first freshness where supported

Ops and `doctor` derive this mode from persisted runtime heartbeats and subscription state rather than from process assumptions.

## Freshness semantics

Freshness is now reasoned from multiple persisted inputs:

- sync state and last successful timestamps
- last trigger source
- receiver heartbeat
- watch heartbeat
- desired vs remote subscription state
- recent accepted and rejected deliveries
- queue backlog
- granted capabilities
- per-family policy windows

This is what allows the app to say “stale because receiver down” or “fresh via webhook” instead of flattening all freshness problems into one label.

## Local-first webhook model

Webhook support remains local-first:

- no hosted relay service exists
- no tunnel orchestration exists
- a real deployment requires a user-managed public HTTPS callback path
- the local runtime still has to be understandable and debuggable without public network access

That is why `webhook replay` and fixture-backed subscription sync are part of the core architecture instead of being treated as optional test utilities.
