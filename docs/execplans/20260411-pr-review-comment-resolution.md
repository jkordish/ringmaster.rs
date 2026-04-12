# PR Review Comment Resolution

## Goal

Address all currently unresolved review threads on PR #8 with compileable fixes, tests, and any required public-surface or docs updates.

## Why

The branch has one functional keybinding bug report plus several API/docs coherence comments that should be resolved before merge.

## Current state

- PR #8 picked up two follow-up review findings after the first resolution pass.
- Shifted character key chords are matched exactly as reported by the terminal.
- The test-store helper still uses the old `open_in_memory` naming even though it reopens an isolated temporary on-disk SQLite database.
- `lib.rs` still exports `ui` and `store` as public modules while recent changes made their internals crate-private.
- Numeric conversion helpers use `num_traits::ToPrimitive` with fallback sentinel values that are harder to reason about than principled saturation.
- Focus activation now emits nested AI actions from the reducer, but the TUI loop still needs to propagate those emitted actions into the async side-effect runner.
- The repo-wide clippy baseline still relies on temporary `multiple_crate_versions` and `too_many_lines` allowances while the dedicated cleanup plans stay open.

## Desired state

- Shifted character chords resolve consistently across terminal normalization differences.
- Test-store helpers describe their actual semantics clearly.
- The public crate surface is explicit and coherent for `store` and `ui`.
- Numeric conversion helpers use predictable saturation behavior and are covered by tests.
- Keyboard activation of AI launch points, preflight controls, and artifact actions triggers the same async work as their direct expert shortcuts.
- `cargo clippy --all-targets --all-features -- -D warnings` stays green for this branch by preserving the documented temporary lint baseline.

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
- `src/app.rs`
- `src/tui.rs`
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
- The temporary crate-level clippy allowances remain until the dedicated dependency-alignment and oversized-function cleanup plans land.
