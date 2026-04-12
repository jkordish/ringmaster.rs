# Dependency Major Upgrade Triage

## Goal

Determine whether `hmac`, `reqwest`, and `sha2` can move to their latest major lines without reintroducing duplicate major-version stacks, while keeping Linux secure storage on the current `keyring`/`secret-service` path unless a stable upstream migration exists.

## Why

`cargo upgrade --incompatible -nv` shows newer major versions are available for several direct dependencies, but the repo has been intentionally favoring a cleaner unified graph over direct-dependency freshness. We need to validate the tradeoff with a real upgrade attempt instead of relying on dry-run output alone.

## Current state

- `reqwest`, `hmac`, and `sha2` are each unified on a single major line across direct and transitive uses.
- `secret-service` is pinned to `4` on Linux alongside `keyring 3.6.3`.
- `src/oura/auth.rs` uses `keyring`, but the crate does not import `secret-service` directly in `src/`.
- The latest `keyring` line is currently `4.0.0-rc.3`, not a stable `4.x` release.
- A real direct-dependency bump experiment for `hmac 0.13`, `reqwest 0.13`, and `sha2 0.11` reintroduced duplicate major stacks immediately:
  - `reqwest 0.12.28` via `oauth2` plus `reqwest 0.13.2` directly
  - `hmac 0.12.1` via `secret-service` plus `hmac 0.13.0` directly
  - `sha2 0.10.9` via `oauth2` / `secret-service` plus `sha2 0.11.0` directly

## Desired state

- Either the direct dependency bumps land cleanly with no new duplicate major stacks, or the repo stays on the current versions with the deferral documented explicitly.
- Linux secure storage remains on the stable `keyring 3.6.3` + `secret-service 4` path.
- The final repo state passes formatting, clippy, tests, and `cargo run -- doctor`.

## Constraints

- Keep the graph unified unless there is a compelling concrete reason not to.
- Do not adopt `keyring 4.0.0-rc.*` by default.
- Keep the branch compilable and documented at the end of the pass.

## Risks

- Direct major bumps can silently reintroduce duplicate majors through `oauth2` and Linux secret-store transitive dependencies.
- `reqwest 0.13`, `hmac 0.13`, or `sha2 0.11` may require API migration work even if the graph split is acceptable.
- Updating the Linux pin comment without checking upstream stability could make the docs less accurate.

## File plan

- `docs/execplans/20260412-dependency-major-upgrade-triage.md`
- `Cargo.toml`
- `Cargo.lock`
- `README.md` only if workflow or dependency-policy wording changes

## Milestones

- [x] Record the experiment and current dependency constraints in a dedicated ExecPlan.
- [x] Apply the direct major bumps for `hmac`, `reqwest`, and `sha2`, then inspect the resulting dependency graph.
- [x] Revert the direct major bumps because the graph split immediately; keep the current unified lines and document the deferral.
- [x] Clarify the `secret-service` pin rationale to reflect the lack of a stable `keyring 4.x` migration path today.

## Verification

- `cargo tree -d`
- `cargo tree -i reqwest`
- `cargo tree -i hmac`
- `cargo tree -i sha2`
- `cargo tree -i secret-service`
- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

## Follow-up work

- Revisit `secret-service 5` only when stable `keyring 4.x` support exists or a stronger product/security reason justifies the migration.
- Monitor upstream `oauth2` and `keyring` releases rather than carrying duplicate stacks locally.
- Re-run the same experiment when either `oauth2` moves to `reqwest 0.13` / `sha2 0.11` or stable `keyring 4.x` lands with a viable Linux Secret Service path.
