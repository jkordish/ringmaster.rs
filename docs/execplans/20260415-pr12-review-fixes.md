# PR #12 Review Fixes

## Goal

Address the actionable review feedback on PR #12 so the branch is safe to merge without leaving auth, sync, migration, and ops regressions behind.

## Why

The review surfaced a handful of correctness issues around blank tokens, SpO2 scope gating, reconcile cadence, legacy rate-limit migration semantics, and follow-on status classification. Those touch multiple module areas and include migration, sync-planning, and UI-fit changes, so they need a tracked plan.

## Current state

The branch already contains the earlier auth/sync/migration/ops/doc fixes plus the reconcile cadence and uppercase badge-abbreviation follow-ups. The final review sweep surfaced three last correctness issues: upsert-only families were still recording successful reconcile coverage for healed windows even though they cannot prune upstream removals within those windows, heartrate reconcile coverage was dropping timestamp precision on the coverage start marker, and sync-state row decoding was redundantly rereading `sync_key` while swallowing a potential SQL error in the fallback family mapping.

## Desired state

PR #12 should:

- fail closed when an access token is blank
- block SpO2 sync when `daily` scope is unavailable
- gate reconcile reruns by the family reconcile window instead of the tail overlap interval
- preserve legacy 429 sync-state rows as rate limits during migration 20
- surface migrated 429 rows as rate-limited in ops and doctor output
- abbreviate uppercase dashboard badges intentionally instead of truncating them
- deduplicate repeated `--family` CLI selections while preserving user order
- keep test-only telemetry scaffold helpers out of non-test builds
- preserve retry coverage for partial daily backfill/reconcile windows while still allowing tail syncs to advance
- only record reconcile coverage for families whose window persistence can truthfully heal missing upstream rows
- preserve full timestamp precision for heartrate reconcile coverage markers
- decode sync-state rows without rereading `sync_key` or masking row errors
- remove stale docs/comments that no longer match the code
- pass repo verification so the unresolved review threads can be resolved confidently

## Constraints

- Preserve unrelated in-flight dashboard work already present in the worktree.
- Keep changes local-first and within existing auth/sync/store/UI boundaries.
- Do not introduce undocumented panic-style error handling in non-test code.

## Risks

- Migration SQL changes can silently misclassify legacy rows if the match is too broad or too narrow.
- Test fixes can accidentally mask the intended review concern if they assert on incidental copy instead of classification.
- Touching `src/app.rs` must avoid interfering with the dashboard cleanup already in progress.

## File plan

- `src/oura/auth.rs`
- `src/oura/sync.rs`
- `src/store/migrations.rs`
- `src/app.rs`
- `src/lib.rs`
- `src/snapshot.rs`
- `src/ui/text_fit.rs`
- `src/ui/telemetry.rs`
- `docs/execplans/20260415-pr12-review-fixes.md`

## Milestones

- [x] Apply the auth/sync/migration/ops/doc fixes from review feedback
- [x] Close the late review follow-ups around duplicate families, test-only scaffolds, and daily retry cursor semantics
- [x] Close the final reconcile-coverage truthfulness gap for upsert-only families
- [x] Re-run compile, lint, test, and doctor verification
- [x] Resolve the remaining actionable PR review threads

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

## Follow-up work

- None.
