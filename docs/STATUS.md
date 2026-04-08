# STATUS.md

## Purpose

This file is the current truth for the repository during the phase-2 MVP live-dashboard-and-refresh pass. It records what now works, what drift was removed, and what remains intentionally deferred.

## Baseline audit at start of this pass

Verified on `2026-04-08` before implementation:

- `cargo fmt --all --check` passed
- `cargo clippy --all-targets --all-features -- -D warnings` passed
- `cargo test --all` passed
- `cargo run -- doctor` passed
- `cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase1` passed

Repository strengths at baseline:

- real local OAuth login and real one-shot sync
- deterministic demo mode and useful snapshot rendering
- SQLite-backed typed store/query seams
- honest live empty/error states
- local-first poll architecture

Repository gaps at baseline:

- the live TUI still felt like a foundation rather than a product
- no long-running refresh scheduler
- `r` in the live TUI was not performing a real refresh
- freshness and capability semantics were still too shallow for daily use
- trends and "what changed" were still closer to placeholders than a genuine insight layer

## Current implemented truth

The repository now includes:

- a useful Dashboard backed by persisted daily rows, freshness badges, capability badges, and derived baseline summaries
- a Timeline screen with date selection, intraday heartrate charting, gap-aware rendering, selected-point details, and a source legend
- a Trends screen with 7d / 30d / 90d windows, daily metric sparklines, baseline-aware summaries, and thin-history confidence notes
- an Ops screen that makes trust explicit: auth state, granted scopes, token metadata, per-family freshness, paths, and active refresh policy
- a reusable scheduler in `src/refresh.rs`
- live background refresh while the TUI is open, without putting sync/auth/store writes on the render path
- `sync watch`, which reuses the same scheduler and sync engine as the live TUI
- durable sync-state backoff fields in SQLite (`failure_count`, `next_attempt_after`)
- deterministic insight helpers for 7d / 30d baselines, day-over-day deltas, and deviation scoring
- fixture-backed scheduler coverage and Ratatui screen coverage for key empty/stale/missing-scope states

## Milestone tracker

- [x] Milestone 1: add the phase-2 plan, migration v4, richer store queries, explicit freshness/availability domain types, and a deterministic insight engine
- [x] Milestone 2: refactor sync into family-selective execution and add the reusable scheduler, `sync watch`, refresh config, and expanded doctor output
- [x] Milestone 3: wire background refresh and snapshot reload into the TUI and productize Dashboard, Timeline, Trends, and Ops
- [x] Milestone 4: add MVP-focused tests and align user-facing docs with the implemented behavior
- [x] Milestone 5: run final verification and repair any failures before closeout

## Verification completed in this pass

Verified on `2026-04-08` after implementation:

- `cargo fmt --all --check` passed
- `cargo clippy --all-targets --all-features -- -D warnings` passed
- `cargo test --all` passed
- `cargo run -- doctor`
- `cargo run -- sync watch --demo --max-iterations 1`

## Known intentional deferrals

- webhook receiver and webhook subscription lifecycle
- broader Oura data surface outside the current personal/daily/heartrate MVP slice
- packaging, installers, and release automation
- richer cross-family correlation and narrative interpretation layers
