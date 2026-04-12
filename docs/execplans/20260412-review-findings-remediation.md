# Review Findings Remediation

## Goal

Resolve the material findings from the workspace review without widening scope beyond the confirmed defects and verification gaps.

## Why

The current tree has two runtime correctness bugs, one cross-cutting date/window reliability issue, a release-hygiene gap in dependency/policy checks, and a second review round that surfaced snapshot compatibility plus data-quality regressions in the new guidance features.

## Current state

- OAuth loopback auth can return on timeout without shutting down the spawned callback server.
- `build_live_state` mutates persisted AI-run rows, and both live snapshot rendering paths reuse it.
- Multiple modules compute `current local day` independently and silently fall back to UTC.
- `cargo audit` reports `RUSTSEC-2026-0097`, and `cargo deny` is not configured in-repo.

## Desired state

- Auth timeout and callback failure paths always clean up the loopback server.
- Observational UI/snapshot entrypoints do not mutate persisted state.
- Local-day resolution comes from one internal helper with one documented fallback policy.
- Dependency/policy checks are green or explicitly configured and justified.

## Constraints

- Keep the app local-first and privacy-first.
- Keep UI rendering pure and avoid leaking store mutation into snapshot-only flows.
- Do not disturb unrelated in-flight work already present in the tree.
- Ship compileable changes with tests.

## Risks

- Touching auth flow can introduce regressions in login success/timeout handling.
- Refactoring live-state construction can accidentally break existing TUI startup semantics.
- Dependency updates may pull in wider graph changes than intended.

## File plan

- `src/oura/auth.rs`
- `src/app.rs`
- `src/lib.rs`
- `src/snapshot.rs`
- `src/store/migrations.rs`
- `src/store/queries.rs`
- `src/review/features.rs`
- `src/oura/models.rs`
- `src/tui.rs` if tests need adjustment
- `Cargo.lock` and possibly `Cargo.toml`
- `deny.toml` if policy configuration is added
- docs if behavior or verification workflows need clarification

## Milestones

- [x] Fix loopback auth cleanup and add timeout cleanup coverage
- [x] Split mutating and non-mutating live-state builders and cover snapshot/read-only paths
- [x] Centralize local-day logic and update affected tests
- [x] Resolve audit/deny gaps and rerun verification
- [x] Restore backward compatibility for previously exported snapshot artifacts
- [x] Load enough comparison-window signal history for weekly activity snapshot trends
- [x] Backfill `daily_sleep.sleep_duration_seconds` during version 17 migration and tighten workout guidance handling

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo test --workspace --all-targets --no-default-features`
- `cargo run -- doctor`
- `cargo audit`
- `cargo deny check`

## Follow-up work

- Reassess public API surface separately if/when the crate is prepared for external consumption.
