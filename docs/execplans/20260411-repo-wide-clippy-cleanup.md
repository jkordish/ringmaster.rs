# Repo-Wide Clippy Cleanup

## Goal

Bring `cargo clippy --all-targets --all-features -- -D warnings` back to green across the repository without adding `#[allow(...)]` shortcuts or weakening the lint bar.

## Why

Clippy is currently failing with a large mix of mechanical, structural, async, and dependency-version diagnostics. The repo already verifies with clippy in its standard workflow, so this pass needs to restore the full quality gate instead of leaving lint debt behind.

## Current state

- `cargo test --all`, `cargo run -- doctor`, and the UI snapshot path are green from the previous navigation pass.
- `cargo clippy --all-targets --all-features -- -D warnings` fails with hundreds of diagnostics.
- The most common lint families are `missing_const_for_fn`, `must_use_candidate`, `missing_errors_doc`, `uninlined_format_args`, `map_unwrap_or`, `needless_pass_by_value`, numeric cast lints, oversized functions, `future_not_send`, and `multiple_crate_versions`.
- Mid-pass update:
  - mechanical doc, by-ref, and numeric-cast cleanup has landed across auth, sync, report, review, snapshot, store, refresh, app, AI, and TUI code paths
  - the non-test `too_many_arguments`, `struct_excessive_bools`, and `derive_partial_eq_without_eq` suppressions have been removed
  - all remaining `#[allow(clippy::...)]` shortcuts in `src/` and `tests/` have now been removed, with test modules rewritten to use explicit helpers and total matches instead of `panic!`/`unwrap`/`expect`-based suppression blocks
  - `cargo check --all-targets --all-features` is green after each batch
  - the current clippy inventory is now concentrated in structural and dependency issues: `too_many_lines`, `future_not_send`, `multiple_crate_versions`, and a small number of remaining mechanical lints such as helper annotations
  - auth and sync now use store-plan reopen flows instead of carrying a live SQLite handle across the core OAuth and Oura fetch awaits, which cut the `future_not_send` inventory from 46 occurrences to 20 on the latest full clippy pass
  - `Store::open_test_store()` now uses an isolated temporary SQLite database so tests remain reopen-safe while the async refactors reacquire short-lived stores after await boundaries
  - the remaining `future_not_send` diagnostics in refresh, report export, webhook subscription management, and library/demo-artifact command helpers have now been eliminated as well; clippy is no longer failing on async `Send` boundaries
  - follow-up mechanical cleanup also removed the lingering auth/test-support pedantic lints that were entangled with the async refactor, leaving the remaining clippy failures concentrated in `too_many_lines`, dependency duplication, and a still-large batch of repetitive test-helper assertions
  - the current full-clippy inventory is down to 55 failures: 32 `too_many_lines` and 23 `multiple_crate_versions`
  - helper extraction landed in `src/app.rs`, `src/tui.rs`, `src/store/queries.rs`, and `src/review/features.rs`, removing several near-threshold test and constructor failures from the `too_many_lines` list
  - an exploratory `keyring` 4.0.0-rc.3 upgrade was attempted and then reverted after confirming that it is not API-compatible with the current auth code and that it expands, rather than shrinks, the transitive graph for this repo
  - a focused structural pass has now landed across `src/app.rs`, `src/lib.rs`, `src/tui.rs`, `src/oura/sync.rs`, and `src/snapshot.rs`; those files are clean under `cargo clippy --all-targets --all-features -- -A clippy::multiple_crate_versions`, and the pass preserved green `cargo check`, `cargo test --all`, and `cargo run -- doctor`
  - the remaining strict-clippy blockers are now split cleanly into two buckets:
    - 23 `multiple_crate_versions` errors that need dependency graph alignment
    - the remaining oversized-function backlog outside the focused pass in `src/ai.rs`, `src/config.rs`, `src/keybindings.rs`, `src/refresh.rs`, `src/report.rs`, `src/review/features.rs`, `src/store/queries.rs`, `src/webhook/receiver.rs`, and one test helper in `src/derive.rs`

## Desired state

- Clippy is green across all targets and features.
- Mechanical lints are fixed directly in code rather than suppressed.
- Async boundaries and dependency versions are corrected where possible instead of hidden.
- Any necessary refactors keep behavior unchanged and preserve the existing green test/doctor/snapshot paths.

## Constraints

- No `#[allow(clippy::...)]`, no `#![allow(...)]`, and no lint-bar lowering.
- Keep the crate compiling after each logical batch.
- Add or update tests when behavior or helper boundaries change.
- Update docs only if commands, dependency posture, or verification workflow change.

## Risks

- Some `missing_const_for_fn` suggestions may not compose cleanly with existing call sites or trait bounds.
- `future_not_send` fixes may expose real cross-thread state issues in auth/derive flows.
- `multiple_crate_versions` may require dependency version alignment rather than local code edits.
- Breaking up oversized functions may touch a lot of reducer or rendering logic and needs careful regression coverage.

## File plan

- `Cargo.toml`
- `src/ai.rs`
- `src/app.rs`
- `src/components/*`
- `src/config.rs`
- `src/derive.rs`
- `src/error.rs`
- `src/eval.rs`
- `src/insights.rs`
- `src/keybindings.rs`
- `src/navigation.rs`
- `src/oura/auth.rs`
- `src/oura/client.rs`
- Additional files from the clippy inventory as needed

## Milestones

- [x] Capture and group the clippy inventory into actionable lint families.
- [x] Fix the first large mechanical lint families across the repo: `# Errors` docs, pass-by-ref cleanup, simple helper extraction, and the current numeric-cast hotspots.
- [x] Complete the focused structural pass over `src/app.rs`, `src/lib.rs`, `src/tui.rs`, `src/oura/sync.rs`, and `src/snapshot.rs`.
- [ ] Reduce the remaining oversized functions outside that focused slice.
- [ ] Execute a dedicated dependency-version alignment campaign for the remaining `multiple_crate_versions` failures.
- [ ] Re-run full strict verification and record the final status.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-nav-ui`

## Follow-up work

- If dependency duplication turns out to require a broader upgrade/downgrade campaign than fits safely in this pass, record the exact blockers and the packages involved instead of suppressing the lint.
- Current dependency blocker notes:
  - `oauth2` 5.0.0 is still the sole source of `thiserror` 1.x and the `rand`/`rand_core`/`rand_chacha` 0.8 line in the graph.
  - `keyring` 3.6.3 remains the root of the `security-framework` 2.x branch and one of the `windows-sys` branches; the attempted 4.0.0-rc.3 migration is not a drop-in replacement today.
  - the remaining platform duplication spans `windows-sys`/`windows-targets` and related architecture crates that are being pulled by otherwise-current upstream dependencies rather than obvious stale direct pins in this crate.
- The remaining `#[allow(clippy::panic)]` and related test-only allowances still need to be removed by rewriting the affected test helpers and assertions rather than reintroducing lint exceptions elsewhere.
- The remaining oversized functions should be reduced by extracting focused builders/helpers instead of weakening the clippy threshold.
