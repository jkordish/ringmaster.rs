# Gap-Safe Oura Sync

## Goal

Implement a per-family Oura sync model that combines incremental freshness with automatic recent-window healing so ringmaster can recover missed data after downtime, webhook gaps, retries, or short-lived failures.

## Why

The current sync path already persists per-slice state, but it still behaves like an incremental watermark system with broad family bundles and no explicit reconcile coverage. That leaves correctness exposed to missed runtime windows and makes operator visibility too coarse.

## Current state

- `sync_selected` runs one pass per selected family and persists one `sync_state` row per sync key.
- Daily sync still bundles `daily_spo2` into `oura.daily`.
- Watch mode already uses periodic sync as the default authority and webhook invalidations as a trigger.
- There is no explicit rolling reconcile watermark, bounded startup catch-up, or manual backfill/reconcile CLI.
- Ops and doctor surfaces show current sync rows, but not family-level reconcile coverage or gap-healing health.

## Desired state

- Each Oura family has durable sync state and policy-driven windows.
- Every normal sync runs an incremental tail plus a rolling reconcile.
- Startup widens the recent catch-up window when downtime exceeded the normal overlap.
- Heartrate remains bounded via chunked catch-up/backfill rather than wide steady-state re-pulls.
- Webhooks accelerate freshness but periodic sync remains the correctness authority.
- Status, ops, and doctor surfaces expose family-level freshness, reconcile coverage, source, and errors.

## Constraints

- Keep the implementation within the current scheduler/webhook/poll-first architecture.
- Preserve local-first behavior and keep UI rendering pure.
- Do not destabilize existing TUI interaction flows beyond the minimum status surface changes.
- Maintain idempotent writes and compileable intermediate states.

## Risks

- Splitting `Spo2` into its own sync family touches sync, webhook, config, and status paths at once.
- Extending `sync_state` without breaking existing reads/tests requires careful migration compatibility.
- Retry and auth refresh hardening can accidentally serialize too much work if scoped too broadly.

## File plan

- `src/oura/sync.rs`, plus a new sync policy/planner module under `src/oura/`
- `src/refresh.rs`
- `src/oura/auth.rs` and possibly `src/oura/client.rs`
- `src/store/migrations.rs`, `src/store/queries.rs`
- `src/cli.rs`, `src/lib.rs`
- `src/app.rs`, `src/components/ops.rs` if needed
- `README.md`, `SPEC.md`, and any sync-specific docs affected by command/config/status changes

## Milestones

- [ ] add the per-family policy/state primitives and migration scaffolding
- [ ] refactor sync execution for tail + rolling reconcile + startup catch-up
- [ ] wire webhook/watch/manual commands into the new family planner
- [ ] surface family-level sync health in doctor and ops/status views
- [ ] add regression coverage and complete verification

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- targeted sync/store/webhook/status tests while iterating
- `cargo run -- doctor`

## Follow-up work

- Revisit whether additional daily-adjacent subfamilies should be split further once the gap-safe core is stable.
- Consider future operator controls for per-family policy overrides in config UX once the baseline correctness model is proven.
