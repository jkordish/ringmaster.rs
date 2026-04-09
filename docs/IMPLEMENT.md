# IMPLEMENT.md

## Purpose

This file is the execution runbook for the current phase-5 product. It only describes flows that work today.

## Commands

Current commands:

```bash
cargo run -- tui
cargo run -- tui --demo
cargo run -- demo
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

## Review flows

The smart layer is deterministic and bounded. There is no freeform chat assistant.

### `review today`

`ringmaster review today`:

1. Loads persisted local data
2. Uses the review signal registry plus `derived_review_signal_days`
3. Builds a ranked daily brief for the anchor day
4. Renders deterministic copy with evidence, counterevidence, confidence, and sufficiency

### `review week`

`ringmaster review week`:

1. Loads persisted local data
2. Aggregates the anchor 7-day window
3. Compares it against a prior 28-day baseline window
4. Renders positive changes, negative drifts, anomalies, and warnings

### `review investigate`

`ringmaster review investigate --focus <readiness|sleep|recovery|stress|activity>`:

1. Loads persisted local data
2. Builds the ranked today and week decks
3. Filters them through the fixed investigation focus
4. Renders a bounded investigation report with:
   - focus-specific evidence
   - counterevidence
   - warnings
   - “look next” pointers

## Review ranking and confidence rules

Ranking is explicit rather than hidden. At a high level, the score combines:

- deviation from baseline
- persistence
- recency
- corroboration
- counterevidence penalty
- freshness penalty
- sufficiency penalty

Today review uses the selected day plus prior comparable history.

Week review uses a trailing 7-day anchor window and a prior 28-day comparison window.

Confidence and sufficiency are separate:

- sufficiency reflects comparable-history volume
- confidence reflects sufficiency plus freshness plus evidence balance

## TUI runtime

`ringmaster tui` is still the live product path. It:

1. Opens the store and reads auth/session metadata
2. Builds an initial `LiveSnapshot`
3. Starts the Ratatui event loop
4. Starts a dedicated background refresh worker on a separate thread
5. Reuses the scheduler core plus `sync_selected(...)` inside that worker
6. Rebuilds a fresh `LiveSnapshot` after each successful refresh
7. Sends snapshot updates back into the reducer as actions

Important boundary:

- widgets never perform HTTP
- widgets never refresh tokens
- widgets never write to SQLite
- the render path stays on persisted presentation models only

## Screen behavior

### Dashboard

- shows the shared selected day
- shows daily metric cards and baseline framing
- shows freshness and capability banners
- reuses top Today review items in the “what likely changed?” surface

### Timeline

- shows a gap-aware intraday heartrate chart
- overlays workouts, enhanced tags, and sessions in separate lanes
- supports family toggles that do not rely on color alone
- shows selected-event details and a selected-day event list

### Trends

- shows 7d / 30d / 90d windows
- shows baseline-aware summaries and thin-history notes
- reuses weekly drift guidance when the ranked week review has strong enough evidence

### Explain

- shows the selected-day summary
- compares the selected day against rolling baselines
- shows evidence bullets and related context entries
- shows caveats for thin data, missing scope, or missing measurement coverage
- surfaces a Review hint when the selected day is part of the ranked brief

### Patterns

- shows descriptive associations by family and metric
- shows `n`, magnitude, and sufficiency bucket
- explicitly says when there is not enough data yet

### Ops

- shows auth/session state
- shows granted capabilities
- shows per-family freshness and trigger-source diagnostics
- shows webhook receiver and subscription health
- shows queue depth, runtime mode, and recent incidents

### Review

- shows Today, Week, and Investigate modes
- shows ranked cards instead of a chart wall
- shows evidence detail, counterevidence, warnings, confidence, and sufficiency
- keeps investigation bounded to fixed focuses instead of freeform questioning

## Shared interaction semantics

- Dashboard, Timeline, Explain, and Review share one selected day
- Timeline and Explain share one selected event
- Timeline, Explain, and Patterns share family filter toggles for workouts, tags, and sessions

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

## Verification sequence

Use this order unless a narrower check is sufficient while developing:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo run -- doctor
cargo run -- review today --demo
cargo run -- review week --demo
cargo run -- review investigate --focus readiness --demo
cargo run -- derive rebuild --demo
```

## Notes for future passes

- Keep UI rendering pure; any new sync, auth, or webhook work belongs outside `src/components/*`.
- Reuse the registry, feature snapshot, and bounded investigation seams instead of inventing parallel smart-summary logic.
- Do not turn Review into chat without a separate explicit design pass.
- Hosted relay services, notifications, packaging, installers, and release automation remain intentionally deferred.
