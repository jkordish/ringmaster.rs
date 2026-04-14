# PR #11 Open Review Thread Resolution

## Goal

Address the still-actionable unresolved review threads on PR #11 and leave the branch with the fixes implemented, verified, and ready for thread resolution.

## Why

The PR still has open review feedback around telemetry availability masking, fixed-width telemetry rendering, snapshot color-mode duplication, and numeric helper edge cases. Those comments are specific enough to fix now and should not be left hanging.

## Current state

- PR #11 has several unresolved threads.
- A subset are already stale in practice or no longer reproduce at `HEAD`.
- The remaining actionable issues cluster around:
  - capability/sync failure masking in availability helpers
  - `micro_histogram` width-contract violations
  - duplicate color-mode handling in UI snapshot commands
  - numeric clamp helpers using pre-clamp fallback logic

## Desired state

- All still-actionable open review items are fixed on this branch.
- Behavior changes are covered by focused tests.
- Verification passes cleanly.
- The resulting fixes are ready to be pushed and the addressed threads can be resolved on GitHub.

## Constraints

- Keep changes targeted to open review feedback.
- Do not reopen unrelated dashboard/layout work while resolving comments.
- Preserve UI/storage/sync boundaries.
- Keep the repo compileable and fully verified.

## Risks

- Availability changes can accidentally overstate or understate failure states across multiple panels.
- Sync/freshness fixes for capability-gated data need to stay narrow so unrelated daily telemetry does not become noisier.
- Small utility changes can ripple into snapshot expectations.

## File plan

- `src/app.rs`
- `src/ui/telemetry.rs`
- `src/lib.rs`
- `src/numeric.rs`
- `src/oura/sync.rs`
- `docs/execplans/20260413-pr11-open-review-thread-resolution.md`

## Milestones

- [x] Map unresolved threads to real `HEAD` behavior and separate stale/non-repro comments
- [x] Implement the actionable fixes with focused tests
- [x] Run verification and prepare the branch for thread resolution

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

## Follow-up work

- If any unresolved threads turn out to be obsolete rather than code-actionable, resolve them on GitHub with a short note instead of forcing speculative code changes.
- Revisit broader sync-status modeling only if the targeted availability fixes still leave ambiguity for capability-gated daily physiology surfaces.
