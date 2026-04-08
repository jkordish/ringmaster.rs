# Phase 1 Hardening and First Real Slice

## Goal

Turn the current bootstrap into a trustworthy phase-1 base for `ringmaster.rs` by landing one real vertical slice end to end:

- `auth login` performs the Oura server-side OAuth flow with loopback callback
- auth/session metadata persists locally while token secrets live in the OS keyring seam
- `sync once` imports real personal info, daily summary, and heartrate data into SQLite
- the TUI and ops views render persisted live state with capability-aware empty, stale, and error states

At the same time, remove bootstrap drift so future passes can build on a repo that feels accurate and production-sane rather than scaffold-heavy.

## Why

The repository already had a strong shell, but the phase-1 value was missing:

- auth was URL planning only
- sync recorded readiness instead of importing data
- docs described scaffolding instead of a real slice
- capability state came from config seams instead of persisted auth truth
- contributor/security/workflow docs were incomplete

Landing the first real slice now gives the next feature pass a stable base with real data flow, real persistence, and fewer fake abstractions.

## Constraints

- Keep the app local-first and single-crate.
- Keep Ratatui rendering pure. No network I/O, token refresh, or database writes from widgets.
- Preserve the central `Event -> Action -> State -> Render` flow.
- No blocking work on the render path.
- No `unwrap`, `expect`, `todo!`, `panic!`, or `dbg!` in non-test code.
- Prefer small, justified dependencies.
- No required webhook infrastructure.
- Prefer end-to-end correctness over visual polish.

## Milestones

- [x] Create this exec plan, add `docs/STATUS.md` and `docs/IMPLEMENT.md`, and harden workflow/docs/templates enough to support the pass.
- [x] Replace scaffolded auth with a persisted loopback OAuth login flow and keyring-backed secret storage seam.
- [x] Replace scaffolded sync with a real personal/daily/heartrate import path plus dry-run and fixture-backed execution.
- [x] Harden capability-aware UI/ops state and add Ratatui screen tests for main live and demo states.
- [x] Run milestone-by-milestone verification, update docs to current truth, and close out deferred work explicitly.

## What changed during execution

- Added `docs/STATUS.md` and `docs/IMPLEMENT.md` as living repo truth/runbook documents.
- Added `CONTRIBUTING.md`, `SECURITY.md`, issue templates, PR template, `.cargo/config.toml` aliases, and a lean CI expansion.
- Replaced scaffolded auth with a real loopback OAuth flow using PKCE, CSRF-safe state validation, denied-auth handling, partial-scope capture, persisted auth metadata, and keyring-backed token storage.
- Replaced scaffolded sync with real phase-1 imports for personal info, daily summaries, and heartrate, including raw payload cache separation, normalized upserts, sync watermarks, and last-error persistence.
- Hardened the live TUI and ops diagnostics around explicit capability, freshness, and failure states.
- Added fixture-backed sync tests and Ratatui `TestBackend` screen coverage.

## Verification

Milestone checks:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`

Final gate:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase1`

## Follow-up work

- Browser-launch ergonomics for `auth login`
- richer trend calculations on top of the now-real daily/heartrate slice
- broader Oura API surface beyond the phase-1 slice
- webhook subscription management and delivery handling
- scheduled/background polling
- packaging and release automation
