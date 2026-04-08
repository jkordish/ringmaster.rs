# STATUS.md

## Purpose

This file is the current truth for the repository during the phase-4 webhook-freshness-and-ops-excellence pass. It records what now works, what operational gaps were removed, and what remains intentionally deferred.

## Baseline audit at start of this pass

Verified on `2026-04-08` before implementation:

- `cargo fmt --all --check` passed
- `cargo clippy --all-targets --all-features -- -D warnings` passed
- `cargo test --all` passed
- `cargo run -- doctor` passed
- `cargo run -- sync watch --demo --max-iterations 1` passed

Repository strengths at baseline:

- real local OAuth login and real one-shot sync
- deterministic demo mode and useful snapshot rendering
- SQLite-backed typed store/query seams
- honest live empty/error states
- local-first poll architecture
- background refresh already reused the same scheduler core as `sync watch`
- derived context overlays, explainability, and pattern summaries were already persisted and testable

Repository gaps at baseline:

- no real webhook receiver or replay path
- no subscription lifecycle management for Oura webhook APIs
- no queue-driven invalidation path inside `sync watch`
- freshness semantics were still too coarse for operations work
- Ops and `doctor` could not explain receiver health, queue lag, or subscription drift

## Current implemented truth

The repository now includes:

- a real `webhook serve` command that verifies Oura webhook traffic, durably records accepted and rejected deliveries, enqueues invalidations, and responds after durable enqueue instead of after sync work
- a declarative webhook subscription surface:
  - `webhook subscriptions list`
  - `webhook subscriptions sync`
- persisted desired subscription specs, remote subscription snapshots, accepted raw deliveries, rejected deliveries, invalidation queue rows, processing attempts, and webhook runtime heartbeats
- a real `webhook replay` path for fixture-backed replay and replaying previously stored deliveries
- a hybrid `sync watch` loop that:
  - consumes pending invalidations first
  - triggers family-aware targeted sync windows
  - preserves scheduled fallback reconciliation
  - keeps unsupported families such as `heartrate` on scheduled-only freshness
- persisted sync trigger provenance, so the app can distinguish webhook-driven freshness from periodic reconcile freshness
- source-aware freshness semantics across the app, Ops, and `doctor`
- a substantially upgraded Ops view that exposes receiver state, callback configuration, subscription health, expiry horizons, delivery history, queue depth, runtime mode, and recent incidents
- a substantially upgraded `doctor` surface that reports webhook readiness, queue visibility, receiver/watch heartbeats, and freshness-risk conditions

## Supported data families

Live sync and persistence currently cover:

- `personal`
- `daily`
- `heartrate`
- `workout`
- `enhanced_tag`
- `session`

Webhook-driven freshness is intentionally limited to the Oura `data_type` surface the app currently supports:

- `daily_sleep`
- `daily_readiness`
- `daily_activity`
- `workout`
- `enhanced_tag`
- `session`

`heartrate` remains scheduler-only because it is not currently exposed as an Oura webhook `data_type`.

## Freshness and ops truth

The app no longer collapses every stale condition into one generic bucket. Families can now resolve to:

- fresh via webhook-driven sync
- fresh via periodic reconcile
- stale because no recent delivery has been seen
- stale because the last sync failed
- stale because the family is unsupported by webhooks
- stale because the receiver is down
- stale because the subscription is missing or expired
- stale because the required capability was not granted
- stale because upstream source data is not yet available

These states are derived from persisted sync state, granted scopes, receiver heartbeat, subscription snapshots, recent deliveries, queue state, and configured freshness policy.

## Webhook and subscription truth

Phase 4 now treats webhook operations as first-class local runtime behavior:

- `webhook serve` is the dedicated HTTP receiver
- `sync watch` is the dedicated queue consumer and scheduler
- desired webhook subscriptions are declared in config
- `webhook subscriptions sync` converges remote state toward local desired state
- `webhook subscriptions sync --dry-run` provides the safe, inspectable default for local verification
- `webhook replay` is the canonical local replay and debugging path

The product remains local-first:

- no hosted relay exists
- no tunnel orchestration exists
- users must provide their own public HTTPS callback path when running a real receiver against Oura

## Milestone tracker

- [x] Milestone 1: add webhook config, schema, typed storage/query support, and the phase 4 execplan/docs scaffolding
- [x] Milestone 2: implement `webhook serve`, `webhook replay`, and declarative subscription list/sync surfaces with fixture-backed coverage
- [x] Milestone 3: integrate invalidation-driven processing into `sync watch`, persist freshness trigger provenance, and preserve scheduled fallback semantics
- [x] Milestone 4: finish the docs sweep, complete full verification, and repair any remaining failures before closeout

## Tests now in place

The phase-4 pass now includes meaningful coverage for:

- migration application for the new webhook and invalidation schema
- webhook verification challenge handling
- signed-delivery acceptance and rejection
- stale timestamp and duplicate delivery handling
- invalidation queue behavior
- fixture-backed subscription sync planning and snapshot persistence
- invalidation-driven targeted sync inside the watch loop
- doctor reporting for webhook readiness, queue state, and runtime heartbeats
- Ops and TUI rendering for the new freshness and operational health states

## Verification completed in this pass

Verified on `2026-04-08` after implementation:

- `cargo fmt --all --check` passed
- `cargo clippy --all-targets --all-features -- -D warnings` passed
- `cargo test --all` passed
- `cargo run -- doctor` passed
- `cargo run -- webhook replay --fixture tests/fixtures/webhooks/sample.json` passed
- `cargo run -- sync watch --demo --max-iterations 1` passed
- `cargo run -- webhook subscriptions sync --dry-run --fixture-dir tests/fixtures/webhooks` passed

## Known intentional deferrals

- hosted relay services
- tunnel orchestration
- packaging, installers, and release automation
- webhook freshness for Oura families the upstream API does not expose as webhook `data_type`s
- push notifications and mobile companion features
- broad theming and non-operational UI polish work
- ML-style interpretation or “AI insights”
