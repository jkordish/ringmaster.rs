# IMPLEMENT.md

## Purpose

This file is the execution runbook for the current phase-7 product. It only describes flows that work today.

## Commands

Current commands:

```bash
cargo run -- tui
cargo run -- tui --demo
cargo run -- demo
cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-ui-snapshots
cargo run -- doctor
cargo run -- auth login
cargo run -- sync once
cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3
cargo run -- sync watch
cargo run -- sync watch --demo --max-iterations 1
cargo run -- derive rebuild
cargo run -- derive rebuild --demo
cargo run -- review today --demo
cargo run -- review week --demo
cargo run -- review investigate --focus readiness --demo
cargo run -- webhook serve
cargo run -- webhook replay --fixture tests/fixtures/webhooks/sample.json
cargo run -- webhook subscriptions list --fixture-dir tests/fixtures/webhooks
cargo run -- webhook subscriptions sync --dry-run --fixture-dir tests/fixtures/webhooks
```

## Visual-system runtime

The TUI now has a dedicated shared presentation layer under `src/ui/*`.

At runtime this means:

1. `src/app.rs` still shapes persisted data into presentation models
2. `src/tui.rs` builds a `UiContext` for the current terminal size
3. screen renderers consume semantic theme, layout, chrome, and chart helpers
4. screens pick compact, medium, or wide choreography instead of squeezing one layout into all terminals

Important boundary:

- widgets never perform HTTP
- widgets never refresh tokens
- widgets never write to SQLite
- the render path stays on persisted presentation models only

## `ui snapshot`

`ringmaster ui snapshot` is the canonical non-interactive design-QA surface. It uses the same rendering stack as the interactive TUI and writes deterministic UTF-8 artifacts to disk.

Supported sources:

- `--demo` for deterministic built-in presentation data
- `--fixture-dir <dir>` for a fixture-backed temporary store
- live local store when neither `--demo` nor `--fixture-dir` is passed

Special fixture-root behavior:

- if `--fixture-dir` points at `tests/fixtures/phase7` or another directory with `strong`, `weak`, and `empty` subdirectories, the command switches into scenario-matrix mode
- scenario-matrix mode expands the same screen and size selection across `strong`, `weak`, `empty`, `stale`, and `error`
- `stale` and `error` are overlaid from the seeded fixture state in code so the repo does not duplicate more Oura payload families than necessary

Supported viewport classes:

- `compact`: `90x28`
- `medium`: `120x36`
- `wide`: `160x44`

Examples:

```bash
cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-ui-snapshots

cargo run -- ui snapshot --demo \
  --screen dashboard --screen timeline --screen review --screen ops \
  --size compact --size wide \
  --out-dir /tmp/ringmaster-ui-snapshots-smoke

cargo run -- ui snapshot \
  --fixture-dir tests/fixtures/phase7 \
  --screen dashboard --screen explain --screen review --screen ops \
  --size compact --size wide \
  --out-dir /tmp/ringmaster-ui-snapshots-phase7-smoke
```

Artifact naming:

- demo and single-fixture mode write one text file per `screen x size` combination as `screen-size.txt`
- phase-7 scenario mode writes one text file per `screen x scenario x size` combination as `screen-scenario-size.txt`

Scenario meanings:

- `strong`: healthy local cache with full capability coverage and rich context
- `weak`: sparse but still usable local history with explicit uncertainty
- `empty`: granted scopes but no cached local records yet
- `stale`: persisted data with degraded freshness or receiver/subscription drift
- `error`: auth, capability, or sync failure state blocking part of the product surface

The command prints the resolved source mode, scenarios, screens, sizes, and generated artifact paths.

## Sync and derivation flows

### `sync once`

`ringmaster sync once` is the one-shot importer. It:

1. Inspects persisted auth/session state
2. Refreshes tokens when needed through the auth layer
3. Fetches the supported live families, including the review-support Oura families
4. Caches raw payloads separately from normalized tables
5. Performs idempotent upserts into SQLite
6. Rebuilds bounded derived state when persisted rows change
7. Updates per-family sync watermarks, status, failure counts, backoff state, structured errors, and trigger provenance

### `sync watch`

`ringmaster sync watch` remains the hybrid invalidation consumer and scheduler. It:

1. Reads persisted sync state and webhook queue state
2. Writes runtime heartbeats
3. Claims pending webhook invalidations first
4. Runs targeted `sync_selected(...)` windows when webhook work exists
5. Preserves scheduled fallback reconciliation for all families
6. Leaves unsupported webhook families honest instead of pretending they are realtime

### `derive rebuild`

`ringmaster derive rebuild` is the explicit non-network rebuild workflow for derived product state. It:

1. Opens the existing SQLite database
2. Reads persisted normalized daily, context, and review-support families
3. Rebuilds canonical context events
4. Rebuilds persisted pattern summaries
5. Rebuilds persisted `derived_review_signal_days`
6. Replaces the derived tables through typed store APIs
7. Prints rebuilt counts for the derived surfaces

This remains the full-history recompute path. Normal syncs still use bounded recent-window rebuilds.

## Screen behavior

### Dashboard

- acts as the editorial front page
- leads with “what matters now”
- follows with the daily metric band
- uses freshness, capability, and drill-down cues as supporting structure

### Timeline

- leads with the chart
- places overlay lanes and selected detail beneath that primary temporal view
- keeps day-event detail available without competing with the chart for first attention

### Trends

- emphasizes 7d / 30d / 90d comparison and baseline drift
- uses compact spark hints and delta language for scanning

### Explain

- starts with the selected-day claim
- then shows measured inputs, evidence, context, and uncertainty
- labels prior-day carryover explicitly so linked context is not mistaken for same-day evidence

### Patterns

- groups deterministic associations and interpretation
- avoids duplicating Explain’s evidence layout

### Ops

- keeps diagnostic density
- separates summary, family status, diagnostics, and warnings more clearly

### Review

- remains the canonical smart surface
- uses ranked cards, bounded brief detail, and fixed-focus investigations
- keeps the selected day and current review mode visible through a lightweight breadcrumb

## Shared interaction semantics

- Dashboard, Timeline, Explain, and Review share one selected day
- Timeline and Explain share one selected event
- Timeline, Explain, and Patterns share family filter toggles for workouts, tags, and sessions
- when new live data arrives, the selected day stays anchored to the same day if possible, then the nearest earlier available day, then the next later day

Default key flow:

- `1-7`: Dashboard, Timeline, Trends, Explain, Patterns, Ops, Review
- `[` / `]`: previous/next selected day on Dashboard, Timeline, Explain, and Review
- `,` / `.`: previous/next heartrate point on Timeline
- `j` / `k`: previous/next selected event on Timeline and Explain
- `j` / `k` on Review: previous/next ranked card
- `w` / `t` / `s`: toggle workouts, tags, and sessions on Timeline, Explain, and Patterns
- `m`: cycle the metric filter on Patterns
- `v`: cycle Review mode
- `f`: cycle Review investigation focus

## Fixture and demo behavior

- `cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3` runs the same parse/normalize pipeline without live credentials and without mutating SQLite
- `cargo run -- sync watch --demo --max-iterations 1` runs the same hybrid scheduler/import path in bounded fixture mode
- `cargo run -- derive rebuild --demo` seeds a temporary store from the phase-5 fixtures and rebuilds all derived tables
- `cargo run -- review today --demo` uses a temporary fixture-backed store and renders the deterministic daily brief
- `cargo run -- review week --demo` uses a temporary fixture-backed store and renders the deterministic weekly brief
- `cargo run -- review investigate --focus readiness --demo` uses the same fixture-backed store and renders the bounded readiness investigation
- `cargo run -- tui --demo` uses deterministic in-memory presentation data and skips live background refresh
- `cargo run -- demo` is an alias for `cargo run -- tui --demo`
- `cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-ui-snapshots` writes deterministic design-review artifacts without requiring a TTY
- `cargo run -- ui snapshot --fixture-dir tests/fixtures/phase7 ...` is the bounded regression path for strong, weak, empty, stale, and error state coverage

## Verification sequence

Use this order unless a narrower check is sufficient while developing:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo run -- doctor
cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-ui-snapshots
cargo run -- ui snapshot --demo \
  --screen dashboard --screen timeline --screen review --screen ops \
  --size compact --size wide \
  --out-dir /tmp/ringmaster-ui-snapshots-smoke
cargo run -- ui snapshot \
  --fixture-dir tests/fixtures/phase7 \
  --screen dashboard --screen explain --screen review --screen ops \
  --size compact --size wide \
  --out-dir /tmp/ringmaster-ui-snapshots-phase7-smoke
```

## Notes for future passes

- keep UI rendering pure; any new sync, auth, or webhook work belongs outside `src/components/*`
- extend the semantic theme/layout/chrome layers instead of scattering style decisions back into individual screens
- keep `ui snapshot` deterministic and text-first unless a later pass explicitly adds a richer visual export path
- do not turn Review into chat without a separate explicit design pass
- hosted relay services, notifications, packaging, installers, and release automation remain intentionally deferred
