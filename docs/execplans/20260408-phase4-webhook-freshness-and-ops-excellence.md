# Phase 4 Webhook Freshness and Ops Excellence

## Goal

Turn the current poll-first product into a near-real-time, operationally trustworthy local-first app by adding real webhook-driven freshness, declarative subscription lifecycle management, invalidation-based sync, and much stronger data-health introspection.

## Why

The current app is useful but still feels poll-first and opaque when freshness goes wrong. This pass is about making freshness more immediate where Oura supports it, preserving honest scheduled fallback where it does not, and giving operators clear local tools to inspect, replay, and repair the freshness pipeline.

## Current state

- `sync watch` is a scheduler-only loop with bounded demo support.
- There is no `webhook` CLI surface.
- Storage has only a placeholder `webhook_subscriptions` table that is not wired into runtime behavior.
- Freshness is coarse (`Fresh`, `Stale`, `NoDataYet`, etc.) and does not explain *why* data is fresh or stale.
- Ops and `doctor` report auth, sync, and record-count basics but do not expose receiver, queue, or subscription health.
- Webhook lifecycle management, replay, and queue-driven sync are intentionally deferred in current docs.

## Desired state

- `webhook serve` runs a dedicated HTTP receiver that verifies Oura deliveries, durably records accepted and rejected deliveries, enqueues invalidations, and responds quickly after durable enqueue.
- `webhook subscriptions list` and `webhook subscriptions sync` provide a declarative, inspectable subscription lifecycle aligned to desired local config.
- `webhook replay` deterministically replays fixtures or stored deliveries through the same verification, enqueue, and bounded processing path.
- `sync watch` becomes a hybrid engine that consumes queued invalidations first, triggers targeted family-specific reconcile windows, and preserves scheduled fallback for all families, especially `heartrate`.
- App state, Ops, and `doctor` expose freshness source, queue state, receiver health, subscription drift, expiry, delivery history, and incident details.
- Docs and tests reflect the real runtime behavior.

## Constraints

- Keep the app local-first and single-crate.
- Preserve the pure UI boundary: no direct HTTP, token refresh, or DB writes from Ratatui widgets.
- `webhook serve` and `sync watch` remain separate command surfaces.
- Receiver acknowledges after durable enqueue, not after sync.
- Do not invent webhook support for unsupported Oura data families; `heartrate` remains scheduled fallback.
- No `unwrap`, `expect`, `todo!`, `panic!`, or `dbg!` in non-test code.
- Use Context7 where library API details materially affect correctness.

## Risks

- Schema expansion can create migration drift or fragile query code if not introduced incrementally.
- Queue-driven sync can cause duplicate storms unless delivery dedupe and invalidation coalescing are explicit.
- Subscription APIs use app credentials, which is a different auth surface than the existing user OAuth flow.
- Freshness semantics can become misleading if trigger provenance and receiver/subscription health are not modeled separately.
- Docs can drift quickly unless updated alongside each milestone.

## File plan

- `Cargo.toml`
- `src/cli.rs`
- `src/config.rs`
- `src/lib.rs`
- `src/error.rs`
- `src/refresh.rs`
- `src/app.rs`
- `src/components/ops.rs`
- `src/oura/client.rs`
- `src/oura/models.rs`
- `src/oura/sync.rs`
- `src/store/db.rs`
- `src/store/migrations.rs`
- `src/store/queries.rs`
- `src/webhook.rs` or `src/webhook/*`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`
- `docs/IMPLEMENT.md`
- `tests/*` and `tests/fixtures/webhooks/*`

## Milestones

- [x] Milestone 1: add webhook config, schema, typed storage/query support, and the phase 4 execplan/docs scaffolding.
- [x] Milestone 2: implement `webhook serve`, `webhook replay`, and declarative subscription list/sync surfaces with fixture-backed tests.
- [x] Milestone 3: integrate invalidation-driven processing into `sync watch`, persist freshness trigger provenance, and preserve scheduled fallback semantics.
- [x] Milestone 4: upgrade Ops and `doctor`, finish documentation, add remaining tests, and run full verification.

## Progress notes

Completed so far:

- webhook schema and typed storage now cover desired subscriptions, remote snapshots, accepted and rejected deliveries, invalidation queue state, processing attempts, and runtime heartbeats
- `webhook serve` now handles verification challenge requests, constant-time signature verification, timestamp checks, accepted and rejected delivery persistence, invalidation enqueue, health endpoints, and clean shutdown
- `webhook replay` now supports fixture envelopes plus replay from stored accepted deliveries
- `webhook subscriptions list` and `webhook subscriptions sync` now support fixture-backed or live inspection, dry-run diffs, renewals, and explicit prune behavior
- `sync watch` now consumes webhook invalidations first, preserves scheduled fallback, and records trigger provenance
- app freshness state, Ops, and `doctor` now reflect receiver health, subscription readiness, delivery history, queue lag, and freshness source
- README, architecture, status, and implementation docs have been updated to match the implemented phase-4 behavior

Closeout status:

- the full required verification sweep completed successfully
- formatting, lint, test, doctor, replay, bounded watch, and dry-run subscription convergence paths all passed
- the remaining work is intentionally deferred product scope, not unresolved failures in this pass

## Verification

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- webhook replay --fixture tests/fixtures/webhooks/sample.json`
- `cargo run -- sync watch --demo --max-iterations 1`
- one bounded dry-run `webhook subscriptions sync` path against a fake or fixture-backed service

## Follow-up work

- Optional live tunnel or reverse-proxy helpers remain explicitly deferred.
- Broader webhook support for new Oura families should only land after Oura exposes stable `data_type` coverage and the app has corresponding sync flows.
- Packaging, installers, and hosted relay workflows remain out of scope for this pass.
