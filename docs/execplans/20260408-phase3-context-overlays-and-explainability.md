# Phase 3 Context Overlays and Explainability

## Goal

Turn the phase-2 MVP into a context-aware, explainable personal observability tool with real context-family sync, canonical derived context events, timeline overlays, an Explain screen, a Patterns screen, and deterministic rebuild workflows.

## Why

The repo already has a trustworthy local-first spine, but it still behaves like a metrics viewer. This pass should make the product useful for day-story exploration and restrained pattern discovery without breaking the purity of the TUI or inventing parallel sync/read paths.

## Current state

- Real auth, real poll-first sync, SQLite persistence, deterministic demo mode, and background refresh are implemented.
- Live sync currently covers `personal`, `daily`, and `heartrate` only.
- Schema placeholders exist for `workouts`, `tags`, `enhanced_tags`, and `sessions`, but the live importer, read models, and UI do not yet use them.
- Timeline overlays are still string placeholders, there is no canonical context-event read model, and there is no derivation rebuild CLI.
- The current insight layer is deterministic and honest, but limited to baseline deltas for a few metrics.

## Desired state

- `workouts`, `enhanced_tags`, and `sessions` sync for real, persist for real, and are covered by fixture-backed tests.
- A persisted derived context-event model unifies context-family rendering and explainability.
- Timeline overlays, selected-event drill-down, day drill-down, Explain, and Patterns are all backed by persisted or derived data rather than ad hoc UI logic.
- `derive rebuild` deterministically recomputes derived overlays, indexes, and pattern summaries from SQLite.
- The app remains local-first, single-crate, poll-first, and pure on the render path.

## Constraints

- Keep the repo single-crate and local-first.
- Reuse the existing auth, sync, scheduler, and store seams rather than inventing parallel paths.
- UI remains pure: no direct HTTP, token refresh, or DB writes from widgets.
- Preserve explicit granted capabilities and one clear refresh-token owner.
- No `unwrap`, `expect`, `todo!`, `dbg!`, or `panic!` in non-test code.
- No webhook receiver implementation in this pass.
- Prefer correctness and honesty over flashy UX or pseudo-intelligence.

## Risks

- Extending sync families can accidentally tangle capability handling if family semantics are not explicit.
- Explain and Patterns can become vague or misleading if wording rules are not enforced in code and tests.
- Timeline overlays can become visually noisy if overlapping events are not normalized into a stable read model first.
- Derived-state rebuilds can drift from live reads if derivation logic is duplicated instead of centralized.

## File plan

- `docs/execplans/20260408-phase3-context-overlays-and-explainability.md`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`
- `docs/IMPLEMENT.md`
- `src/cli.rs`
- `src/lib.rs`
- `src/config.rs`
- `src/action.rs`
- `src/app.rs`
- `src/tui.rs`
- `src/components/*`
- `src/refresh.rs`
- `src/insights.rs`
- `src/oura/models.rs`
- `src/oura/client.rs`
- `src/oura/sync.rs`
- `src/store/migrations.rs`
- `src/store/queries.rs`
- `tests/*`
- `tests/fixtures/phase3/*`

## Milestones

- [x] Milestone 1: add the phase-3 exec plan, schema/config/scheduler foundations, and real sync/store coverage for `workouts`, `enhanced_tags`, and `sessions`.
- [x] Milestone 2: add persisted derivation for canonical context events and pattern summaries plus the `derive rebuild` CLI path.
- [x] Milestone 3: unify selected-day and selected-event state across the TUI, upgrade Timeline with overlays and drill-down, and add Explain + Patterns screens.
- [x] Milestone 4: add meaningful migration/sync/derivation/analytics/UI tests and update docs continuously to match reality.
- [x] Milestone 5: run the full verification sequence, repair failures, and leave the repo compileable and documented.

## Verification

After each milestone:

- `cargo fmt --all --check`
- `cargo test --all`

### Milestone 1 checkpoint

- Added real sync families, config knobs, fixture-backed demo data, and typed store APIs for `workouts`, `enhanced_tags`, and `sessions`.
- Verified with:
  - `cargo fmt --all --check`
  - `cargo test --all`
  - `cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3`

### Milestone 2 checkpoint

- Added persisted `derived_context_events` and `derived_pattern_summaries` tables plus typed store/query APIs.
- Added `derive rebuild` with `--demo` / `--fixture-dir` support and deterministic rebuild tests.
- Verified with:
  - `cargo fmt --all --check`
  - `cargo test --all`
  - `cargo run -- derive rebuild --demo`

### Milestone 3 checkpoint

- Reworked app state to share selected-day semantics across Dashboard, Timeline, and Explain.
- Added shared selected-event semantics across Timeline and Explain.
- Replaced placeholder timeline overlays with real family lanes for workouts, enhanced tags, and sessions.
- Added selected-event details, selected-day event lists, and filter toggles that do not rely on color alone.
- Added dedicated Explain and Patterns screens with deterministic, evidence-based output.
- Verified with:
  - `cargo test --all`
  - `cargo clippy --all-targets --all-features -- -D warnings`

### Milestone 4 checkpoint

- Added and updated tests for migration, sync, derivation, app-state behavior, overlay rendering, Explain rendering, Patterns insufficient-data states, and non-interactive TUI smoke rendering.
- Updated `README.md`, `docs/ARCHITECTURE.md`, `docs/STATUS.md`, and `docs/IMPLEMENT.md` to describe the implemented phase-3 behavior rather than the phase-2 MVP.

Before final closeout:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3`
- `cargo run -- derive rebuild --demo`
- one documented non-interactive TUI smoke path (`cargo run -- demo`)

### Final verification completed

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3`
- `cargo run -- derive rebuild --demo`
- `cargo run -- demo`
- `cargo run -- sync watch --demo --max-iterations 1`

## Follow-up work

- webhook invalidation feeding the existing scheduler
- additional Oura families beyond the phase-3 scope
- richer exports and sharing workflows
- packaging and release automation
- more advanced but still honest descriptive analytics once broader history exists
