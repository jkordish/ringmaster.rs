# PR Review Comment Resolution

## Goal

Address all currently unresolved review threads on PR #8 with compileable fixes, tests, and any required public-surface or docs updates.

## Why

The branch has one functional keybinding bug report plus several API/docs coherence comments that should be resolved before merge.

## Current state

- PR #8 has five unresolved review threads.
- Shifted character key chords are matched exactly as reported by the terminal.
- The test-store helper still uses the old `open_in_memory` naming even though it reopens an isolated temporary on-disk SQLite database.
- `lib.rs` still exports `ui` and `store` as public modules while recent changes made their internals crate-private.
- Numeric conversion helpers use `num_traits::ToPrimitive` with fallback sentinel values that are harder to reason about than principled saturation.

## Desired state

- Shifted character chords resolve consistently across terminal normalization differences.
- Test-store helpers describe their actual semantics clearly.
- The public crate surface is explicit and coherent for `store` and `ui`.
- Numeric conversion helpers use predictable saturation behavior and are covered by tests.

## Constraints

- Keep the repo compileable throughout.
- Avoid widening the public API accidentally.
- Add or update tests for behavior changes.
- Keep docs in sync with any naming or API-surface changes.

## Risks

- Keybinding normalization could accidentally collapse distinct bindings if done too aggressively.
- Public-surface cleanup could break internal call sites if visibility changes are incomplete.
- Numeric helper changes could subtly affect derived statistics if saturation behavior is wrong.

## File plan

- `docs/execplans/20260411-pr-review-comment-resolution.md`
- `src/keybindings.rs`
- `src/lib.rs`
- `src/store/db.rs`
- `src/numeric.rs`
- `Cargo.toml` if dependency cleanup is justified

## Milestones

- [x] Capture the comment set and translate it into concrete code changes.
- [x] Implement the fixes and update targeted tests/docs.
- [x] Run verification, update the plan, and resolve the review threads.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

## Follow-up work

- If PR #8 picks up additional comments during the pass, either fold them into this plan or record them explicitly as a separate follow-up.
- Full `cargo clippy --all-targets --all-features -- -D warnings` still fails on the repo's pre-existing `multiple_crate_versions` and `too_many_lines` backlog outside this patch.
