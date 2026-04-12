# ARCHITECTURE.md

## Scope

This document describes the implemented architecture for `ringmaster.rs` as of `2026-04-12`. It reflects the code that exists in the repository today, including the typed evidence registry and claims policy, the population-aware guidance resolver and sensitive-metric runtime guards, the snapshot library, AI run registry, report export workflow, the in-app eval lab/regression console, and the navigation/focus/keybinding standardization that now anchors the TUI shell.

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
- optional external synthesis only through explicit exported snapshots
- structured machine-safe AI outputs instead of prose parsing
- durable local artifact workflows instead of transient AI stdout
- a shared scientific evidence contract that governs deterministic copy, reports, and AI outputs

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

snapshot export
  -> store + auth/session seams
  -> derived read models + bounded view queries
  -> evidence-registry descriptors + guidance metadata
  -> privacy-profile redaction layer
  -> deterministic versioned JSON bundle
  -> snapshot manifest + provenance persistence

snapshot list / snapshot show
  -> snapshot catalog queries
  -> path-or-id resolution
  -> compact metadata rendering
  -> lineage and privacy visibility

ai review / ai compare
  -> local snapshot file loading only
  -> provider boundary (`dry_run`, `fixture`, `openai`)
  -> canonical request builders with versioned prompt/task/schema framing
  -> evidence-registry-aware prompt constraints
  -> OpenAI Responses API with strict JSON schema output when enabled
  -> local post-provider sanitation against the claims policy
  -> local artifact persistence + summary cache + request fingerprint
  -> local human-readable briefing rendering

ai runs list / ai runs show
  -> AI run registry queries
  -> local artifact inspection over time

report export
  -> source resolution from snapshot or AI run
  -> shared report document model
  -> evidence-strength and safety-rail sections
  -> Markdown / HTML renderers
  -> report manifest persistence

ai eval
  -> fixture manifest loading
  -> deterministic snapshot/artifact fixture validation
  -> local grader execution
  -> persisted manifest/case/grader/lineage detail assembly
  -> optional JSON summary export
  -> eval summary + detail persistence

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
  -> background AI launch/preflight/report side-effect worker tasks
  -> top-level AI workbench + inline launch points
  -> pure screen renderers

ui snapshot
  -> app state builder
  -> deterministic screen + size matrix selection
  -> optional scenario matrix expansion
  -> AI workbench smoke rendering
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

### `src/evidence/*`

Responsibilities:

- define the canonical evidence registry for surfaced metrics and claim classes
- define the supported population profiles plus per-claim support and fallback behavior
- classify claims by evidence tier, evidence type, interpretation scope, and numeric-threshold policy
- define allowed wording templates, prohibited wording categories, and required caution rails
- provide compact `EvidenceDescriptor` values for snapshots, reports, and AI artifacts
- validate registry completeness and provenance expectations

Non-responsibilities:

- database I/O
- HTTP or model calls
- direct widget rendering

This module is the scientific contract for the product. Deterministic UI, reports, and AI outputs may consume it, but they must not fork their own parallel evidence rules.

### `src/config.rs`

Responsibilities:

- XDG-friendly path resolution
- config file parsing from `config.toml`
- environment overrides
- runtime directory creation
- Oura/logging defaults
- guidance profile defaults and environment/file overrides
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

Current AI config covers:

- provider enablement
- model selection
- reasoning effort
- timeout and retry policy
- stateless vs stateful mode
- inline vs file-upload input transport
- prompt cache mode
- optional `safety_identifier`
- env-var based API key loading

### `src/app.rs`

Responsibilities:

- screen enum and navigation state
- explicit freshness and availability modeling
- demo/live application snapshots and presentation models
- shared selected-day and selected-event state
- shared selected-review state
- user-facing status/footer text
- active population profile and support-state shaping for screen models
- shaping store/auth/derived/webhook data into screen-specific models
- deterministic selected-day summaries and pattern rows
- deterministic review decks and bounded investigations

The app layer is where persisted normalized rows, derived tables, auth diagnostics, sync provenance, subscription state, delivery history, queue state, and runtime heartbeats become screen models. It deliberately does not own terminal I/O, HTTP, or SQL.

The optional AI layer does not bypass this shaping logic. Snapshot exports are built from typed store and derived queries, not from raw SQL dumps or live database inspection.

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
- `LiveSnapshot`: the immutable persisted-data snapshot sent into the reducer after each background refresh, including evidence-registry version and stale-evidence runtime state for Ops/doctor visibility
- `WebhookOpsSnapshot`: persisted receiver/subscription/delivery/queue/runtime view data shaped for the Status screen and `doctor`
- `selected_day_index`: shared by Dashboard, Timeline, Explain, and Review
- selected-day continuity preserves the exact selected day when possible, then the nearest earlier available day, then the next later day, before falling back to the newest day
- `selected_event_id`: shared by Timeline and Explain
- `focused_region`: the currently focused major region for the active screen
- `screen_focus_memory`: per-screen region restoration memory used when returning to a screen from `Views`
- `focused_top_nav_screen`: the currently focused item in the visible top-level `Views` tab row
- `help_open`: scoped help-overlay state
- `search`: current-context search state, including query, match counts, and prior region for focus restore
- `overlay_filters`: shared family toggles for workouts, tags, and sessions
- `selected_overlay_toggle_index`: shared focused overlay-selector state for Timeline, Explain, and Patterns
- `window_hours`: selected Timeline chart window preset
- `PatternMetricFilter`: shared pattern metric filtering for the Patterns screen
- `review_mode`: Today, Week, or Investigate within the Review screen
- `review_focus`: readiness, sleep, recovery, stress, or activity within Investigate mode
- `selected_review_card_index`: selected ranked card within Review
- `ai_preflight`: explicit in-app send gate state for AI launches
- `ai_preflight_control`: focused preflight control inside the transient confirm/privacy/cancel row
- `ai_browser_tab`: shared browser state for saved `runs`, `snapshots`, `reports`, and `evals`
- `selected_ai_run_index`, `selected_snapshot_catalog_index`, `selected_report_export_index`, `selected_ai_eval_run_index`: stable saved-artifact list selection state inside the AI workbench
- `selected_ai_artifact_action_index`: focused action inside the visible AI artifact-action pane
- `ai_artifacts_by_day`: preloaded day-keyed summaries derived from `ai_artifacts` joined through `snapshot_exports`, used for Review provenance display

### `src/tui.rs`

Responsibilities:

- interactive Ratatui event loop
- terminal session lifecycle
- key-event routing through the centralized binding registry
- live background refresh worker wiring
- AI preflight preparation, launch orchestration, cancellation, report-export side effects, and local evidence jump routing
- deterministic snapshot rendering via `TestBackend`
- shared frame chrome driven by semantic theme and viewport context
- visible orientation strip plus scoped help/search overlays

Why snapshot rendering exists:

- keeps demo mode useful without a TTY
- supports stable tests and CI smoke checks
- reuses the same component tree as the interactive UI
- gives the repo a canonical scenario-matrix QA path without duplicating widget logic

The shared frame now owns the design-system-driven shell:

- semantic header and active-screen treatment
- visible `Views` tabs on wide layouts
- orientation strip with focused-region cues
- contextual footer generated from the binding registry
- centralized compact/medium/wide viewport context
- shared panel/badge/divider language used by all screens
- region-ordered back-out so reducer-level `Esc` handling always unwinds the previous major region on the active screen before returning to `Views`

How background refresh works:

- the main loop stays focused on terminal input, tick events, reducer updates, and rendering
- a dedicated worker thread owns a single-thread Tokio runtime
- that worker opens the store on its own thread, uses the shared scheduler core from `src/refresh.rs`, and calls the same `sync_selected(...)` path as `sync once`
- when new persisted data is available, the worker rebuilds a `LiveSnapshot` and sends `Action::LiveSnapshotLoaded` back to the UI loop
- this keeps blocking store/auth/sync work off the render path and avoids `Send` pressure on the SQLite + sync stack

How background AI work works:

- widgets only emit `Action`s such as launch, confirm, rerun, follow-up, report export, and evidence jump
- `handle_ai_side_effect(...)` in `src/tui.rs` owns the non-render-path orchestration for AI preflight generation, AI execution, cancellation, and report export
- expensive preflight preparation runs inside `spawn_blocking(...)`
- AI execution runs in async tasks and persists lifecycle transitions through `ai_runs`
- report export is executed off the render path through a blocking worker boundary so SQLite-backed context loading never stalls the frame loop
- running tasks communicate back to the reducer exclusively through `Action`s such as `AiPreflightPrepared`, `AiPreflightFailed`, `RefreshFailed`, and `LiveSnapshotLoaded`

Implemented screen set:

- Dashboard
- Timeline
- Trends
- Explain
- Patterns
- Review
- AI
- Status

There is intentionally no freeform AI chat screen in this pass. The `AI` screen is a guided workbench for snapshot-bounded review, compare, follow-up, and report flows, and the TUI remains a pure consumer of persisted local state plus explicit user-triggered side effects.

### `src/navigation.rs`

Responsibilities:

- canonical major-region ordering per screen
- typed navigation movement semantics
- pane-type semantics for selectors, lists, chart/pager regions, and detail panes
- truthful region labels for promoted controls such as Timeline window presets, overlay selectors, and AI artifact actions
- honest focus-stop definitions so read-only subpanels stay inside a screen body region instead of becoming fake keyboard regions
- search-scope definitions
- transient-layer definitions
- focused-control labels used by chrome and reducer logic

Boundary rules:

- this module is pure and state-free
- it does not know about rendering, database handles, network work, or provider state

### `src/keybindings.rs`

Responsibilities:

- centralized keybinding registry
- scope-aware binding lookup
- standard vs expert alias separation
- footer/help generation support
- collision detection coverage through tests

Implemented scopes:

- `Global`
- `Screen`
- `Region`
- `ScreenRegion`
- `Transient`

Implemented behavior:

- transients override region and screen bindings
- `Tab` / `Shift+Tab` are reserved for major-region traversal
- arrows, `Home`, `End`, and paging keys are used inside composites
- function keys are intentionally excluded from the standard model

### `src/components/ai.rs`

Responsibilities:

- render the dedicated AI workbench surface
- render the unified list/detail browser for snapshots, AI runs, and reports
- render launch points and trust defaults
- render the visible AI artifact-action pane
- render the preflight overlay as a compact, legible confirmation gate with a visible control row

Non-responsibilities:

- provider calls
- database reads or writes
- token refresh
- file export side effects

### `src/snapshot.rs`

Responsibilities:

- canonical snapshot bundle types
- snapshot scope resolution
- privacy-profile redaction
- deterministic serialization and hashing
- snapshot manifest + provenance record creation
- snapshot file loading and validation

Boundary rules:

- snapshot export reads typed store/query outputs and derived artifacts only
- snapshot export never reaches into auth secrets, raw config internals, or live provider state
- provenance references are local-only join handles and remain opaque inside exported artifacts

Implemented concepts:

- `SnapshotBundleV1`
- `PrivacyProfile::{Redacted,Balanced,Full}`
- `SnapshotMetadata.active_population_profile`
- `ResolvedSnapshotScope`
- manifest persistence in `snapshot_exports`
- local export-reference mapping in `snapshot_provenance_refs`
- catalog summary fields for freshness, trust, capability, and provenance

### `src/ai.rs`

Responsibilities:

- provider abstraction for snapshot review and compare
- dry-run, fixture, and OpenAI provider implementations
- canonical request construction
- Structured Outputs schema generation
- local briefing rendering
- population-aware artifact metadata and post-provider sanitization
- persisted AI artifact record construction

Boundary rules:

- AI code only reads local snapshot files, never the live store directly
- provider configuration is isolated from sync/auth/webhook logic
- no OpenAI-specific behavior is allowed inside Ratatui widgets
- rendered prose is derived locally from structured JSON, not parsed back from model text

Implemented concepts:

- `ReviewArtifactV1`
- `CompareArtifactV1`
- provider metadata and run modes (`real`, `dry_run`, `fixture`)
- prompt and schema version constants
- stateless-by-default Responses API usage with no tools
- request previews and request fingerprints

### `src/ai_prompts.rs` and `src/ai_prompts/*`

Responsibilities:

- versioned prompt and task-frame asset loading
- centralized prompt/schema version names
- keeping prompt strings out of unrelated implementation modules

Boundary rule:

- prompt assets define framing, not runtime transport or persistence behavior

### `src/report.rs`

Responsibilities:

- source resolution for report export
- shared `ReportDocument` view model construction
- Markdown and HTML rendering
- report manifest persistence and lineage wiring

Boundary rule:

- report rendering is derived from local snapshots and AI artifacts only
- no network work happens in report generation

### `src/eval.rs`

Responsibilities:

- fixture manifest loading
- deterministic local artifact evaluation
- grader execution and summary scoring
- eval summary persistence
- persisted manifest/case/grader/linkage detail payload construction
- optional JSON export

Boundary rule:

- eval runs do not require live OpenAI calls
- eval fixtures remain snapshot-first and local-only
- historical eval browsing reads only persisted local detail; it does not rerun fixtures from the TUI

### `src/ui/*`

Responsibilities:

- semantic palette and emphasis roles
- breakpoint-aware spacing and layout helpers
- reusable chrome/panel/badge builders
- shared chart grammar
- deterministic multi-screen snapshot artifact generation
- scenario tagging for `strong`, `weak`, `empty`, `stale`, and `error`

Implemented modules:

- `src/ui/theme.rs`: semantic palette roles, tones, and emphasis helpers
- `src/ui/layout.rs`: viewport classes and layout helpers
- `src/ui/chrome.rs`: section titles, panels, badges, and focus/state affordances
- `src/ui/charts.rs`: shared line/bar/spark styling and annotation helpers
- `src/ui/snapshot.rs`: deterministic `ui snapshot` artifact writing

The snapshot writer now supports two naming modes:

- single-source/demo mode: `screen-size.txt`
- scenario mode: `screen-scenario-size.txt`

Boundary rule:

- these modules are presentation-only and do not own persistence, auth, sync, or network work

### `src/components/*`

Responsibilities:

- pure rendering for Dashboard, Timeline, Trends, Explain, Patterns, Review, and Status
- screen-specific choreography using shared semantic theme, layout, and chrome helpers

Boundary rule:

- components receive presentation models only
- no network calls
- no SQLite handles
- no token refresh logic

Component responsibilities today:

- Dashboard renders the editorial front page: “what matters now,” the daily metric band, freshness/capability framing, and drill-down cues, including locally persisted HRV, respiratory-rate, and `spo2` physiology panels
- Timeline renders the chart-first temporal composition, overlay lanes, selected detail, and selected-day event list
- Trends renders the comparative scanning matrix with windows, deltas, spark hints, and baseline readouts
- Explain renders a telemetry-first evidence flow: claim, measured inputs, supporting evidence, context, and uncertainty
- Timeline, Explain, and Review include lightweight breadcrumbs only when they keep shared day or event context visible
- Patterns renders grouped associations, reading guidance, and interpretation through the same telemetry-first panel vocabulary while remaining distinct from Explain
- Status renders the utilitarian operator console with summary, family status, diagnostics, and warnings without reaching back into the store
- Review renders ranked briefing cards, bounded investigation detail, warnings, and a small read-only AI artifact panel through the shared telemetry panel language without making network or database calls

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
- `snapshot_exports`
- `snapshot_provenance_refs`
- `ai_artifacts`
- `report_exports`
- `ai_eval_runs`

Important query responsibilities added in phase 5:

- normalized upserts and views for the six review-support Oura families
- persisted review signal snapshots rebuilt from local data
- typed reads for ranked review and investigation inputs

Additional query responsibilities added in this pass:

- snapshot catalog list/show queries keyed by stable snapshot hash
- local export-reference provenance lookup for AI evidence mapping
- persisted AI review/compare artifact storage and latest-artifact lookup
- day-scoped AI artifact summary lookup keyed by snapshot `anchor_day`, including compare-side lineage resolution
- report export manifest persistence and lineage lookup
- eval summary and `details_json` persistence

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
- richer Status-oriented read surfaces for subscriptions, deliveries, queue depth, and incidents

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
- token secrets live behind the `SecretStore` seam; production defaults to the keyring-backed backend, Linux expects a desktop Secret Service provider, headless users can explicitly opt into a file-backed token store, and tests use in-memory or temp-file stores
- the capability model now tracks Oura's broader scope surface, including `email`, `spo2`, `ring_configuration`, `stress`, and `heart_health`, so auth and ops surfaces can distinguish between granted, missing, and wired local access
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
  - `/v2/usercollection/sleep`
  - `/v2/usercollection/sleep_time`
  - `/v2/usercollection/daily_spo2`
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

Status and `doctor` derive this mode from persisted runtime heartbeats and subscription state rather than from process assumptions. The same runtime view now also surfaces evidence-registry versioning and stale-review health so scientific maintenance is visible alongside sync/auth/webhook health.

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
