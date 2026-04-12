# Dependency Version Alignment

## Goal

Reduce the remaining `clippy::multiple_crate_versions` failures by aligning the dependency graph where that is realistically possible, while documenting any ecosystem splits that cannot be collapsed safely in this crate today.

## Why

The focused structural refactor pass is complete for the largest reducers and renderers in `src/app.rs`, `src/lib.rs`, `src/tui.rs`, `src/oura/sync.rs`, and `src/snapshot.rs`. Strict clippy is now blocked primarily by dependency duplication rather than local code shape, so version alignment needs to be treated as its own campaign instead of being mixed into UI and sync refactors.

## Current state

- `cargo check --all-targets --all-features`, `cargo test --all`, and `cargo run -- doctor` are green after the structural pass.
- `cargo clippy --all-targets --all-features -- -D warnings` still fails on 23 `multiple_crate_versions` diagnostics.
- The current duplicate families are:
  - `bitflags`
  - `core-foundation`
  - `foldhash`
  - `getrandom`
  - `hashbrown`
  - `r-efi`
  - `rand`
  - `rand_chacha`
  - `rand_core`
  - `security-framework`
  - `syn`
  - `thiserror`
  - `thiserror-impl`
  - `windows-sys`
  - `windows-targets`
  - the related `windows_*` architecture crates
- Known root causes already identified in the cleanup pass:
  - `oauth2` 5.0.0 is the current source of the `thiserror` 1.x line and the older `rand` family.
  - `keyring` 3.6.3 plus its `secret-service` / `zbus` chain pulls the older `security-framework` and one of the `windows-sys` branches.
  - current runtime crates such as `clap`, `tokio`, and `crossterm` pull the newer `windows-sys` line.
  - some duplicates may be unavoidable proc-macro or target-platform splits and need to be classified before changing versions.

## Desired state

- The dependency graph is reduced where safe and practical.
- Every remaining duplication is either:
  - removed by version alignment, or
  - explicitly documented as an upstream or platform split that this crate cannot realistically collapse yet.
- Cargo and code changes preserve the current auth, keyring, sync, and TUI behavior.

## Constraints

- Do not weaken the lint bar with `#[allow(clippy::multiple_crate_versions)]`.
- Keep the repo compiling between upgrade attempts.
- Prefer root-cause grouping over package-by-package churn.
- Do not regress local-first auth storage behavior or the existing Linux keyring fixes.
- Avoid broad dependency churn that mixes unrelated feature work into this campaign.

## Risks

- Some duplicate lines may come from incompatible upstream major versions rather than stale direct pins.
- Moving `oauth2`, `keyring`, or transitive auth/security crates may require code changes in sensitive auth flows.
- Platform-specific `windows-*` crate duplication may not be fully solvable from this crate if upstreams pin different target families.
- A naive update may expand the graph or regress compatibility, as already seen with the attempted `keyring` 4.0.0-rc.3 migration.

## File plan

- `Cargo.toml`
- `Cargo.lock`
- `docs/execplans/20260411-repo-wide-clippy-cleanup.md`
- Any auth or integration files that need small API fixes from dependency upgrades:
  - `src/oura/auth.rs`
  - `src/config.rs`
  - other targeted modules only if an upgrade forces an API adaptation

## Milestones

- [ ] Classify each duplicate family by root cause using `cargo tree`, `cargo tree -i`, and direct dependency review.
- [ ] Separate “safe to align” duplicates from likely unavoidable upstream/platform splits.
- [ ] Apply the smallest viable version changes for the safe-to-align groups and repair any API fallout.
- [ ] Re-run strict clippy to measure what remains and document any irreducible duplication explicitly.

## Verification

- `cargo check --all-targets --all-features`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo clippy --all-targets --all-features -- -D warnings`

## Follow-up work

- If some duplicate families are upstream-locked, record the exact parents and why they cannot be unified yet.
- If an auth or keyring upgrade requires a larger API migration, split that into a follow-up execplan instead of folding it into ad hoc Cargo edits.
