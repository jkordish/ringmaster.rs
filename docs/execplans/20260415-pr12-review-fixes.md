# PR #12 Review Fixes

## Goal

Address the actionable review feedback on PR #12 so the branch is safe to merge without leaving auth, sync, migration, and ops regressions behind.

## Why

The review surfaced a handful of correctness issues around blank tokens, SpO2 scope gating, reconcile cadence, legacy rate-limit migration semantics, and follow-on status classification. Those touch multiple module areas and include migration, sync-planning, and UI-fit changes, so they need a tracked plan.

## Current state

The branch now contains the earlier auth/sync/migration/ops/doc fixes plus two final follow-up fixes: reconcile cadence now needs to be gated by the reconcile window instead of overlap, and uppercase dashboard badges need case-insensitive abbreviation matching. Verification has been rerun cleanly and the remaining work is closing the two outstanding review threads.

## Desired state

PR #12 should:

- fail closed when an access token is blank
- block SpO2 sync when `daily` scope is unavailable
- gate reconcile reruns by the family reconcile window instead of the tail overlap interval
- preserve legacy 429 sync-state rows as rate limits during migration 20
- surface migrated 429 rows as rate-limited in ops and doctor output
- abbreviate uppercase dashboard badges intentionally instead of truncating them
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
- `docs/execplans/20260415-pr12-review-fixes.md`

## Milestones

- [x] Apply the auth/sync/migration/ops/doc fixes from review feedback
- [x] Re-run compile, lint, test, and doctor verification
- [ ] Resolve the remaining actionable PR review threads

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

## Follow-up work

- None planned beyond resolving the review threads once verification is green.
