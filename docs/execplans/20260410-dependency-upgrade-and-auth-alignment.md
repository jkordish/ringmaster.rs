# Dependency Upgrade And Auth Alignment

## Goal

Upgrade the crate to the latest viable dependency releases while preserving the recent Oura OAuth and Linux secure-storage fixes.

## Why

The repo is behind on several direct dependencies, and the current upgrade attempt is partially applied and broken. We need to finish the upgrade cleanly so the branch is buildable, lint-clean, and aligned with Oura's current OAuth expectations.

## Current state

- Compatible version bumps are partially applied in `Cargo.toml`.
- Manual major-version bumps for `reqwest`, `hmac`, and `sha2` are also partially applied.
- The branch currently fails because `reqwest` 0.13 changed TLS feature names.
- Local fixes for Oura callback URLs, Oura `tag` scope handling, and Linux keyring behavior are present but not yet folded into a clean dependency pass.
- Linux secure storage currently depends on a direct `secret-service` runtime feature pin that may be constrained by `keyring`.

## Desired state

- `Cargo.toml` and `Cargo.lock` reflect the latest safe direct dependency set we can support today.
- Oura auth defaults and Linux secure-storage behavior remain correct after the upgrade.
- The project builds, passes clippy with `-D warnings`, passes tests, and runs `doctor`.
- Any dependency that cannot move because of an upstream compatibility constraint is intentionally pinned and documented.

## Constraints

- Keep the system local-first and snapshot-first.
- Do not regress the current Oura OAuth callback/scope fixes.
- Do not force a broken `secret-service` major mismatch if `keyring` still requires v4.
- Keep the branch green at the end of the pass.

## Risks

- `reqwest` 0.13 may require feature and API adjustments.
- `hmac` and `sha2` majors may introduce trait or digest API changes.
- Linux secure-storage can regress if the `keyring` and `secret-service` runtime features drift apart.

## File plan

- `docs/execplans/20260410-dependency-upgrade-and-auth-alignment.md`
- `Cargo.toml`
- `Cargo.lock`
- `src/config.rs`
- `src/oura/auth.rs`
- `src/oura/models.rs`
- Any Rust modules that need API fixes from dependency updates
- `README.md`
- `docs/ARCHITECTURE.md`

## Milestones

- [x] finish the direct dependency upgrade strategy and resolve upstream version constraints
- [x] fix code for any major-version API fallout while preserving Oura auth/keyring behavior
- [x] run full verification and document any intentionally pinned dependencies

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo build`
- `cargo run -- doctor`

## Follow-up work

- Revisit `secret-service` once `keyring` supports the newer major without requiring a duplicate runtime stack.
