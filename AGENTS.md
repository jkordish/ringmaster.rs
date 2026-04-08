# AGENTS.md

## Mission

Build `ringmaster.rs`: a local-first Rust terminal application for exploring Oura Cloud data with a polished Ratatui interface, strong architecture, and production-sane defaults.

## Read this first

For any non-trivial task, read these files in this order:

1. `AGENTS.md`
2. `SPEC.md`
3. `GOVERNANCE.md`
4. `README.md`
5. `docs/EXECPLAN.md` when the task spans multiple files, changes architecture, or takes more than about 30 minutes

Do not shotgun-read every markdown file in the repo. Read additional docs only when the task requires them.

## Non-negotiables

- Keep the project local-first and privacy-first.
- Favor a single-package crate at bootstrap time. Do not split into a workspace without a clear need.
- Keep UI rendering pure. Components must not perform network I/O, database writes, or token refresh.
- Poll-first for v1. Webhooks may be scaffolded, but webhook infrastructure must not be required for the first usable release.
- Ship compileable changes. Do not leave the repo broken between milestones.
- No `unwrap`, `expect`, `todo!`, `panic!`, or `dbg!` in non-test code unless the task explicitly calls for it and the reason is documented.
- Add or update tests for behavior changes.
- Update docs when architecture, commands, configuration, or workflows change.

## Architecture boundaries

- `src/cli.rs`: CLI parsing and command routing
- `src/app.rs`, `src/tui.rs`, `src/action.rs`: app state, event loop, screen flow
- `src/components/*`: pure UI components and per-screen state
- `src/oura/*`: OAuth, API client, models, sync orchestration
- `src/store/*`: storage layer, migrations, and query boundaries
- `docs/*`: design notes, plans, architecture, roadmap

### Boundary rules

- UI components may depend on app state and presentation structs, but not on HTTP clients or database handles.
- Sync code may write to the store, but it should not know about Ratatui widgets.
- Store code should expose typed operations, not leak SQL details into the UI.
- Avoid giant god structs. Prefer narrow interfaces and explicit data flow.

## Working style

For new features, schema changes, auth work, or multi-file refactors, write an ExecPlan in `docs/execplans/YYYYMMDD-<slug>.md` using `docs/EXECPLAN.md` before implementation.

Keep plans alive:
- update the plan when scope changes
- mark milestones complete as you go
- note risks and follow-up work

If the code and spec disagree, fix the code and update the spec in the same change unless the task explicitly says otherwise.

## Decision defaults

Unless the task says otherwise, prefer these defaults:

- Rust stable, edition 2024
- `ratatui + crossterm + tokio` for the TUI
- `clap` for CLI parsing
- `reqwest + oauth2 + axum` for Oura auth/client surfaces
- `rusqlite` for the bootstrap storage layer
- `serde` for config and API types
- `tracing` + structured logs for observability
- minimal dependencies until each one is justified by running code

## Verification

Run the narrowest relevant checks during development, then the full suite before concluding major work:

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

When finishing a task, report exactly what changed, what was verified, and any remaining gaps.

