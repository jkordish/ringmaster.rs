# CODEX_START_PROMPT.md

Paste the following into Codex after committing the seed files in this repo.

---

Read `AGENTS.md`, `SPEC.md`, `GOVERNANCE.md`, `README.md`, and `docs/EXECPLAN.md` first. Then bootstrap this repository into a real phase-0 / phase-1 foundation for `ringmaster.rs`.

## Goal

Turn this seed repo into a compileable, testable, documented Rust application shell for a local-first Oura TUI built with Ratatui. Focus on architecture, correctness, and developer experience. Do **not** try to finish the full product in one pass.

## Product constraints

- This is a **local-first** tool, not a cloud service.
- The app target is Oura Cloud API v2 with OAuth2 for user data.
- **Poll-first v1**. Webhooks may be scaffolded behind interfaces, but webhook infra must not be required for the first usable release.
- UI components must stay pure: no direct network calls, token refresh, or DB writes from widgets.
- Prefer a **single-package crate** for now.
- Prefer `rusqlite` over `sqlx` at bootstrap time unless there is a compelling reason not to.
- Keep dependencies tight and justified.
- No `unwrap`, `expect`, `todo!`, `panic!`, or `dbg!` in non-test code.

## Deliverables

Implement the following in one coherent pass:

1. **Real CLI**
   - use `clap`
   - support subcommands:
     - `tui`
     - `doctor`
     - `auth login`
     - `sync once`
     - `demo`

2. **Real app shell**
   - use `ratatui`, `crossterm`, and `tokio`
   - create a TUI event loop with app state and actions
   - provide at least these screens/components:
     - Dashboard
     - Timeline
     - Trends
     - Ops
   - keyboard navigation can be minimal, but the structure should be clean and extendable

3. **Demo mode**
   - deterministic fake data
   - enough data to exercise the dashboard, trends, and ops screens
   - runnable without Oura credentials or network access

4. **Config + diagnostics**
   - app config loading with sane defaults
   - XDG-friendly paths where practical
   - `doctor` should print resolved paths, config presence, database location, and basic health info
   - structured logging via `tracing`

5. **Storage foundation**
   - SQLite database
   - migration runner
   - schema scaffolding for:
     - app metadata
     - sync state
     - raw payload cache
     - daily summary families
     - heartrate
     - workouts
     - tags / enhanced tags
     - sessions
   - typed store module boundaries

6. **Oura integration seams**
   - typed client interfaces
   - OAuth loopback server scaffold or partial implementation
   - sync orchestration scaffold
   - explicit support for missing scopes / partial capability
   - leave clean hooks for webhook support later, but do not require it to function

7. **Docs**
   - update `README.md` to match the implemented bootstrap
   - add `docs/ARCHITECTURE.md`
   - update `SPEC.md` only if implementation forces a better decision than the current spec

8. **Verification**
   - make sure these pass:
     - `cargo fmt --all`
     - `cargo clippy --all-targets --all-features -- -D warnings`
     - `cargo test --all`
     - `cargo run -- doctor`

## Execution process

1. First create an ExecPlan at `docs/execplans/YYYYMMDD-bootstrap-foundation.md`.
2. Then implement in milestones so the repo stays compileable.
3. Prefer replacing the placeholder std-only skeleton with real code rather than layering hacks on top of it.
4. If the spec is ambiguous, choose the simplest production-sane option and document it.
5. If you must defer anything, list it under a clearly named follow-up section in docs instead of scattering TODOs through the code.

## Quality bar

The final state should feel like a serious repo, not a demo dump:
- clean module boundaries
- coherent naming
- good errors
- real tests
- no fake abstractions that do not earn their keep
- docs aligned with reality

At the end, give a concise summary:
- what you changed
- what commands you ran
- what passed
- what remains for the next milestone

---
