# Sync Outcome And Shutdown Hardening

## Goal

Make background and watch-driven sync execution safe under shutdown/interrupt, and align daily partial-sync semantics so optional endpoint degradation does not create invalidation retry loops or silently age out retry windows.

## Why

The current sync flow can drop a state-mutating sync future on shutdown, leaving persisted rows ahead of sync-state bookkeeping and derived rebuilds. Separately, daily syncs marked `partial` currently both advance the cursor and fail webhook invalidations, which creates contradictory retry behavior.

## Current state

- TUI background refresh can stop waiting for an in-flight sync as soon as shutdown is requested.
- Watch-mode periodic reconcile uses a cancelable helper that drops the sync future on ctrl-c.
- Daily slice reports use `SyncRunStatus::Partial` when optional endpoints degrade, but still store the end-of-window watermark.
- Webhook invalidation settlement only treats `Success` as completion.

## Desired state

- Once a sync begins mutating state, shutdown and ctrl-c wait for that sync to finish durably before exiting.
- Daily `Partial` continues to mean "core daily data is current, optional support endpoints degraded".
- Partial daily syncs preserve a retryable cursor/window for optional data instead of aging those gaps out.
- Webhook invalidations complete successfully when their family reconcile is complete, even if optional daily endpoints degraded.

## Constraints

- Keep the fix local to existing modules; avoid workspace or major architectural churn.
- Preserve existing UI/reporting behavior that relies on `SyncRunStatus::Partial` for optional daily capability warnings.
- Maintain local-first behavior and avoid render-path auth or store mutation changes.

## Risks

- Changing shutdown semantics can alter worker lifecycle expectations and tests.
- Changing partial watermark behavior can increase overlap re-fetches if implemented too broadly.
- Broadening invalidation success semantics too far could hide real failures.

## File plan

- `src/tui.rs`
- `src/refresh.rs`
- `src/oura/sync.rs`
- `src/store/queries.rs` if helper methods are the cleanest place to centralize semantics
- `README.md` or architecture docs only if behavior contracts need documentation
- relevant tests in the touched modules

## Milestones

- [x] codify the desired sync outcome semantics in a narrow helper/API
- [x] make TUI/watch shutdown wait for in-flight sync completion
- [x] preserve daily partial retry windows and settle invalidations correctly
- [x] add regression tests for shutdown durability and partial daily webhook handling
- [x] rerun targeted and full verification, then mark the plan complete

## Verification

- `cargo fmt --all`
- `cargo test --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- targeted tests for `src/tui.rs`, `src/refresh.rs`, and `src/oura/sync.rs`
- `cargo run -- doctor`

## Follow-up work

- Consider a future explicit sync-outcome model that separates durability, freshness, and degraded optional capability coverage instead of encoding all three in one enum.
