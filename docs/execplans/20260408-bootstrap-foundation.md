# Bootstrap Foundation

## Goal

Replace the placeholder seed with a real phase-0 / phase-1 foundation for `ringmaster.rs`: a compileable local-first Rust CLI/TUI shell with deterministic demo mode, SQLite storage, Oura integration seams, diagnostics, and aligned documentation.

## Why

The current repository is intentionally minimal and string-based. Before product features can land safely, the project needs real command parsing, runtime wiring, storage initialization, a Ratatui event loop, and typed module boundaries that keep UI, sync, auth, and persistence concerns separate.

## Current state

- `Cargo.toml` has no runtime dependencies.
- Commands are manually parsed and only return placeholder strings.
- The TUI is not a real terminal application.
- Store and Oura modules are stubs without runnable migrations or orchestration.
- Docs describe the intended direction more than the implemented system.

## Desired state

- `clap`-driven CLI with `tui`, `doctor`, `auth login`, `sync once`, and `demo`.
- `tokio` + `ratatui` + `crossterm` app shell with a minimal but real event loop and screen navigation.
- Deterministic demo data that exercises Dashboard, Timeline, Trends, and Ops.
- Config/environment resolution with XDG-friendly paths and structured logging.
- SQLite database bootstrap with migration runner and typed store surfaces.
- Oura auth/client/sync interfaces that support poll-first operation and partial capability/scopes.
- Docs that match the implemented bootstrap.

## Constraints

- Keep the project local-first and privacy-first.
- Stay as a single-package crate.
- Use `rusqlite` at bootstrap time unless a compelling reason appears.
- UI components stay pure and do not perform network calls, token refresh, or DB writes.
- Poll-first v1: webhook support may be scaffolded behind interfaces only.
- No `unwrap`, `expect`, `todo!`, `panic!`, `dbg!` in non-test code.
- Keep dependencies tight and justified by running code.

## Risks

- TUI/runtime wiring can sprawl if app state and component responsibilities are not kept narrow.
- SQLite initialization and migration code can leak SQL details into unrelated layers.
- OAuth scaffolding can overreach phase-1 scope if it tries to finish the full auth system.
- Docs may drift if implementation choices differ from the initial placeholder descriptions.

## File plan

- `Cargo.toml`
- `src/main.rs`
- `src/lib.rs`
- `src/cli.rs`
- `src/config.rs`
- `src/error.rs`
- `src/action.rs`
- `src/app.rs`
- `src/tui.rs`
- `src/components/*`
- `src/oura/*`
- `src/store/*`
- `README.md`
- `docs/ARCHITECTURE.md`
- `SPEC.md` if implementation clarifies or improves a decision

## Milestones

- [x] Add runtime dependencies and replace the placeholder command flow with a real CLI/bootstrap entrypoint.
- [x] Implement config, logging, database path resolution, migration runner, and typed store scaffolding.
- [x] Implement Oura auth/client/sync seams with poll-first orchestration boundaries and partial capability handling.
- [x] Build the Ratatui application shell with deterministic demo-backed Dashboard, Timeline, Trends, and Ops screens.
- [x] Update docs, add tests, and run full verification including `cargo run -- doctor`.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- Manual smoke check of `cargo run -- demo`

## Follow-up work

- Complete a production OAuth loopback flow and local secret storage integration.
- Implement real Oura API requests and normalization for daily summaries, heart rate, workouts, tags, and sessions.
- Add scheduled/background polling and richer freshness interpretation.
- Add webhook subscription storage and delivery handling when poll-first v1 is stable.
