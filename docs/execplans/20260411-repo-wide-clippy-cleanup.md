# Repo-Wide Clippy Cleanup

## Goal

Bring `cargo clippy --all-targets --all-features -- -D warnings` back to green across the repository, and document any intentionally retained crate-wide exceptions instead of leaving the lint posture ambiguous.

## Why

Clippy is currently failing with a large mix of mechanical, structural, async, and dependency-version diagnostics. The repo already verifies with clippy in its standard workflow, so this pass needs to restore the full quality gate instead of leaving lint debt behind.

## Current state

- `cargo clippy --all-targets --all-features -- -D warnings` is green again.
- The repo-wide cleanup removed the failing mechanical, async-boundary, and stale-surface diagnostics that originally blocked the release gate.
- The current crate-wide lint posture is explicit instead of implicit:
  - `clippy::multiple_crate_versions` remains allowed at crate root because the remaining duplicates are driven by upstream dependency graph constraints rather than stale direct pins in this crate.
  - `clippy::too_many_lines` remains allowed at crate root because the remaining oversized modules are a refactor-sized follow-up, not a correctness blocker for this pass.
- `cargo check --all-targets --all-features`, `cargo test --all`, and `cargo run -- doctor` all remain part of the release gate for this cleanup.

## Desired state

- Clippy is green across all targets and features.
- Mechanical lints are fixed directly in code rather than suppressed.
- Async boundaries and dependency versions are corrected where possible instead of hidden.
- Any intentionally retained crate-wide exceptions are documented in both code and plan notes.
- Any necessary refactors keep behavior unchanged and preserve the existing green test/doctor/snapshot paths.

## Constraints

- Do not introduce new ad hoc `#[allow(clippy::...)]` suppressions in implementation code or tests.
- If a crate-wide exception must remain, document why it is still needed and keep it narrowly scoped.
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
- [x] Reduce the remaining strict-clippy blockers enough to restore a green gate without adding new local suppressions.
- [x] Reconcile the remaining crate-wide exceptions in documentation so the lint posture is explicit.
- [ ] Execute a dedicated dependency-version alignment campaign for the remaining `multiple_crate_versions` duplication.
- [ ] Break up the remaining oversized modules so the crate-root `too_many_lines` exception can be removed.
- [x] Re-run full strict verification and record the final status.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-nav-ui`

Final status on 2026-04-14:
- `cargo fmt --all` passed
- `cargo clippy --all-targets --all-features -- -D warnings` passed
- `cargo test --all` passed
- `cargo run -- doctor` passed

## Follow-up work

- `oauth2` 5.0.0 is still the sole source of `thiserror` 1.x and the `rand`/`rand_core`/`rand_chacha` 0.8 line in the graph.
- `keyring` 3.6.3 remains the root of the `security-framework` 2.x branch and one of the `windows-sys` branches; the attempted 4.0.0-rc.3 migration is not a drop-in replacement today.
- The remaining platform duplication spans `windows-sys`/`windows-targets` and related architecture crates that are being pulled by otherwise-current upstream dependencies rather than obvious stale direct pins in this crate.
- The crate-root `clippy::too_many_lines` allowance should be removed by extracting focused helpers from the remaining oversized modules rather than by weakening per-function thresholds.
- The crate-root `clippy::multiple_crate_versions` allowance should be removed only once the upstream dependency graph can be aligned without destabilizing auth, keyring, or platform support.
