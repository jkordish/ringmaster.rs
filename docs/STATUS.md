# STATUS.md

## Purpose

This file is the current truth for the repository during the phase-1 hardening and first-real-slice pass. It records what is implemented, what drift was removed, and what remains intentionally deferred.

## Baseline audit at start of this pass

Verified on `2026-04-08` before implementation:

- `cargo fmt --all --check` passed
- `cargo clippy --all-targets --all-features -- -D warnings` passed
- `cargo test --all` passed
- `cargo run -- doctor` passed

Repository strengths at baseline:

- real CLI/TUI shell with deterministic demo mode
- SQLite migrations and typed store boundaries
- poll-first architecture seams between UI, auth/sync, and store
- honest empty-state rendering in live mode
- basic CI for formatting, linting, and tests

Repository drift at baseline:

- `auth login` was still a planning flow, not a persisted OAuth login
- `sync once` recorded readiness instead of importing live data
- granted scopes came from config/bootstrap seams instead of persisted auth truth
- `docs/STATUS.md` and `docs/IMPLEMENT.md` were missing
- contributor/security/templates were missing
- docs still described scaffolded auth/sync behavior rather than a real vertical slice

## Current implemented truth

The repository now includes:

- real loopback OAuth login with PKCE, state validation, denied-auth handling, partial-scope capture, and persisted auth metadata
- keyring-backed runtime secret storage with an in-memory test seam
- real `sync once` imports for personal info, daily sleep, daily readiness, daily activity, and heartrate
- raw payload caching separated from normalized tables
- per-slice sync state with watermarks and last structured Oura problems
- capability-aware Dashboard, Timeline, Trends, and Ops rendering
- fixture-backed sync and Ratatui screen tests for the main live states
- lean contributor/security docs and CI that includes doctor plus fixture sync smoke

## Milestone tracker

- [x] Milestone 1: workflow/docs/templates hardening
- [x] Milestone 2: persisted OAuth login and secret storage seam
- [x] Milestone 3: real sync slice for personal info, daily summaries, and heartrate
- [x] Milestone 4: capability-aware UI/ops hardening and screen tests
- [x] Milestone 5: final docs alignment and full verification

## Verification completed at end of this pass

Verified on `2026-04-08` after implementation:

- `cargo fmt --all --check` passed
- `cargo clippy --all-targets --all-features -- -D warnings` passed
- `cargo test --all` passed
- `cargo run -- doctor` passed
- `cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase1` passed

## Known intentional deferrals

- webhook receiver and subscription delivery
- broader Oura data surface outside the phase-1 slice
- scheduled/background sync
- packaging and release automation
