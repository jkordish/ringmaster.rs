# Phase 2 MVP Live Dashboard and Refresh

## Run note

This file remains the source of truth for the current run.

As of the latest audit on `2026-04-08`, the phase-2 MVP described below is already implemented in the repository. The current run therefore focuses on:

- re-validating the delivered MVP against the requested scope
- refreshing docs/plan state when needed
- rerunning the required verification sequence
- repairing only concrete regressions or drift discovered during that audit

## Goal

Turn the phase-1 foundation into a daily-drivable MVP with:

- a useful live TUI over persisted Oura data
- explicit freshness and missing-data semantics
- a small deterministic insight layer
- incremental background refresh while the TUI is open
- a reusable `sync watch` scheduler path for debugging and automation

## Why

The repo now has real auth, real sync, and persisted local data, but the product still feels like a foundation instead of a tool someone would run every day. This pass should make the app trustworthy and useful without broadening scope beyond the current Oura slice.

## Current state

- `sync once` is real for personal info, daily summaries, and heartrate.
- The TUI renders persisted store data, but most live views are still placeholders or proof-of-pipeline summaries.
- `r` in the TUI is a no-op in live mode.
- There is no long-running refresh scheduler.
- Freshness/error semantics are present, but not modeled explicitly enough to distinguish stale, empty, missing scope, auth failure, source delay, and never-synced states.

## Desired state

- Dashboard, Timeline, Trends, and Ops are useful with real persisted data.
- A reusable scheduler drives background refresh in `tui` and `sync watch`.
- The app models and displays fresh/stale/unavailable state per family with clear reasons.
- A small insight engine produces 7d/30d baselines, day-over-day deltas, deviation indicators, and compact summary text.
- Sync keeps durable watermarks/backoff state and remains idempotent.
- Docs and doctor output reflect the MVP behavior accurately.

## Constraints

- Keep the app local-first and single-crate.
- Keep Ratatui rendering pure. No network I/O, token refresh, or database writes from widgets.
- Preserve the central `Event -> Action -> State -> Render` flow.
- No blocking work on the render path.
- Reuse the existing sync engine instead of inventing a second import path.
- No `unwrap`, `expect`, `todo!`, `panic!`, or `dbg!` in non-test code.
- Do not add webhook receiver infrastructure in this pass.

## Risks

- `rusqlite::Connection` is not shareable across async tasks, so background work must open the store inside worker tasks.
- Timeline charting can sprawl if it grows beyond one-day heartrate visualization.
- Freshness semantics can become confusing if scheduler timing and source lag are mixed together in the UI.
- It is easy to overfit “insights”; this pass must stay deterministic and modest.

## File plan

- `src/action.rs`
- `src/app.rs`
- `src/cli.rs`
- `src/config.rs`
- `src/lib.rs`
- `src/tui.rs`
- `src/components/*`
- `src/oura/sync.rs`
- `src/store/migrations.rs`
- `src/store/queries.rs`
- `src/refresh.rs`
- `src/insights.rs`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`
- `docs/IMPLEMENT.md`

## Milestones

- [x] Milestone 1: add the phase-2 plan, migration v4, richer store queries, explicit freshness/availability domain types, and a deterministic insight engine.
- [x] Milestone 2: refactor sync into family-selective execution and add the reusable scheduler, `sync watch`, refresh config, and expanded doctor output.
- [x] Milestone 3: wire background refresh and snapshot reload into the TUI and productize Dashboard, Timeline, Trends, and Ops.
- [x] Milestone 4: add MVP-focused tests and align user-facing docs with the implemented behavior.
- [x] Milestone 5: run full verification and repair any failures before closeout.

## Verification

After each milestone:

- `cargo fmt --all --check`
- `cargo test --all`

Before final closeout:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- sync watch --demo --max-iterations 1`

## Follow-up work

- webhook invalidation feeding the same scheduler
- broader Oura families beyond personal/daily/heartrate
- richer overlays and correlations
- packaging and release automation

## Audit results for this run

- [x] Re-read the requested docs and current implementation in the required order.
- [x] Confirm the requested MVP surfaces already exist in code: useful Dashboard, Timeline, Trends, Ops, background refresh, `sync watch`, explicit freshness semantics, and deterministic insights.
- [x] Re-run the required verification sequence and confirm the repo remains green.
