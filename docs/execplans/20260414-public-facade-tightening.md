# 20260414 public facade tightening

## Goal

Align `src/lib.rs` with the documented app-first contract by exposing only the supported library surface and keeping internal app/UI/sync/store modules crate-private.

## Why

The package is now marked `publish = false` and the README/lib docs both say the library facade is intentionally small, but `src/lib.rs` still publicly exports almost every internal module. That leaves the codebase advertising a much larger API than intended and makes future refactors look semver-sensitive even for an app-first crate.

## Current state

- `Cargo.toml` marks the package as non-publishable.
- `README.md` and `src/lib.rs` describe a small supported facade.
- `src/lib.rs` still exports most internal modules publicly.

## Desired state

- The public facade matches the documented contract:
  - `run_from`
  - CLI parsing types under `ringmaster::cli`
  - top-level `Result` / `RingmasterError`
- Internal app, UI, sync, store, and webhook modules are crate-private.
- The supported facade is pinned by a compileable example or test.

## Constraints

- Keep the binary and existing integration tests working.
- Avoid broad compatibility shims for APIs we do not intend to support.
- Preserve compileable, documented behavior on the current branch.

## File plan

- `src/lib.rs`
- `README.md`
- `docs/execplans/20260414-public-facade-tightening.md`

## Milestones

- [ ] Narrow the public library surface to the documented facade
- [ ] Add a compileable public-facade anchor
- [ ] Re-run verification and confirm the branch stays clean

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
