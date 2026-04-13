# PR #11 Final Review Thread Closure

## Goal

Resolve the remaining unresolved actionable review threads on PR #11 with targeted fixes and regression coverage.

## Why

The branch is down to a small set of concrete UI and telemetry-state review comments. Closing them now keeps the PR reviewable and avoids carrying known polish/correctness gaps into follow-up work.

## Current state

- PR #11 still has unresolved threads across Timeline focus/layout, shared lines-panel metrics in Explain/Patterns, and dashboard telemetry availability.
- The branch already contains the earlier `Resting HR` failure-state fix and one resolved review thread.

## Desired state

- Timeline shows a single focused shell at a time and preserves non-compact hero detail.
- Explain and Patterns use viewport-level shell metrics rather than panel-width-derived metrics.
- Dashboard breakdown and weekly heatmap availability preserve real telemetry state instead of collapsing to placeholder states.
- The new behavior is covered by focused tests/snapshots where practical.

## Constraints

- Keep changes targeted to the open review comments.
- Preserve the shared shell/title-row architecture rather than introducing one-off panel behavior.
- Keep the repo compileable and fully verified.

## Risks

- Focus-state changes can accidentally disrupt keyboard navigation or snapshot expectations.
- Availability changes can overstate degraded telemetry if they bypass existing helper semantics.
- Shared panel metric changes can subtly shift layout in multiple screens.

## File plan

- `src/components/timeline.rs`
- `src/components/patterns.rs`
- `src/components/explain.rs`
- `src/app.rs`
- tests in `src/app.rs` and any affected snapshot/unit coverage

## Milestones

- [x] Map unresolved review threads to current `HEAD` behavior
- [x] Implement the remaining targeted fixes
- [x] Run verification and leave the branch ready for thread resolution

## Verification

- `cargo fmt --all`
- `cargo test --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo run -- doctor`

## Follow-up work

- If any remaining review thread turns out to be stale after these fixes, resolve it on GitHub with a short note rather than forcing speculative code churn.
