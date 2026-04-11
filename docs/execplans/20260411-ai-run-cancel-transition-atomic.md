# Atomic AI run cancellation/interruption transitions

## Goal
Prevent cancel/interruption paths from overwriting a run that has already completed between the status read and the status write.

## Why
The current TUI helper reads an AI run, derives a terminal cancellation/interruption record, then unconditionally upserts it. That leaves a race with `run_ai_job` where a run can succeed after the read but still be overwritten as cancelled or interrupted.

## Current state
- `src/tui.rs` reads the current run via `ai_run(run_id)` and writes the transitioned record with `upsert_ai_run`.
- `src/store/queries.rs` exposes unconditional AI run upserts but no conditional persisted-status guard.
- Existing tests prove stale UI snapshots do not cancel succeeded runs, but they do not prove the write itself is guarded atomically.

## Desired state
- The TUI cancel/interruption helper only persists a terminal transition if the stored row is still `queued` or `running` at write time.
- If the run already transitioned to a terminal state, the cancellation/interruption path should return `false` and leave the persisted row untouched.
- Regression coverage should lock in the conditional store update behavior and the TUI-facing result.

## Constraints
- No schema changes.
- Keep the fix local-first and narrow.
- Preserve existing run metadata when the transition is allowed.

## Risks
- Duplicating AI run update SQL could drift from the main upsert path if fields change later.
- A too-broad conditional update could block valid terminal transitions.

## File plan
- `docs/execplans/20260411-ai-run-cancel-transition-atomic.md`
- `src/store/queries.rs`
- `src/tui.rs`

## Milestones
- [x] Add a conditional AI run update helper in the store layer.
- [x] Route TUI cancel/interruption transitions through the conditional helper.
- [x] Add regression coverage and verify with fmt/test/clippy/doctor.

## Verification
- `cargo fmt --all`
- `cargo test --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo run -- doctor`

## Follow-up work
- If we add more run-state transitions later, consider consolidating unconditional upsert and guarded update SQL behind a shared internal builder to reduce duplication.
