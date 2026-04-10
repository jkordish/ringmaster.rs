# Phase 7 Scenario Hardening And State Coverage

## Goal

Harden `ringmaster.rs` with scenario-rich fixture coverage, deeper state QA, sharper uncertainty and missing-capability copy, and better selected-day continuity without adding new Oura families.

## Why

Phase 6 improved the visual language, but snapshot QA is still mostly demo-first and shallow. We need canonical strong, weak, empty, stale, and error scenarios across every major screen so regressions in copy, state continuity, and terminal layouts are caught before they ship.

## Current state

- `ringmaster ui snapshot` can render demo or a single fixture-backed state.
- Snapshot filenames are screen-and-size only.
- Screen regression tests cover a few representative states, not a full scenario matrix.
- Selected-day restoration after live snapshot replacement falls back to the newest day instead of the nearest available continuity point.
- Explain, Timeline, and Review still make some degraded states harder to interpret than they need to be.

## Desired state

- `ui snapshot` supports a canonical phase-7 fixture root and emits a scenario matrix.
- Every major screen has deterministic `strong`, `weak`, `empty`, `stale`, and `error` snapshot coverage across compact and wide layouts.
- Copy for uncertainty, missing capability, and empty local-state cases is more precise and less ambiguous.
- Selected-day continuity prefers the nearest earlier day, then the next later day, before falling back to newest data.
- Explain and other linked screens expose breadcrumbs only where they materially reduce cognitive load.

## Constraints

- Do not add new Oura families.
- Keep UI rendering pure and keep scenario shaping outside widget code.
- Keep `ringmaster ui snapshot --demo ...` behavior intact.
- Keep the repo compileable after each milestone.
- Update docs continuously as behavior and workflows change.

## Risks

- Snapshot plumbing changes could break the existing demo path or artifact naming.
- Scenario overlays could drift away from realistic live state if they bypass too much of the normal snapshot pipeline.
- More explicit breadcrumbs could add clutter on compact terminals if they are not carefully scoped.

## File plan

- `docs/execplans/20260409-phase7-scenario-hardening-and-state-coverage.md`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`
- `docs/IMPLEMENT.md`
- `src/app.rs`
- `src/cli.rs`
- `src/lib.rs`
- `src/tui.rs`
- `src/ui/snapshot.rs`
- `src/components/explain.rs`
- `src/components/review.rs`
- `src/components/timeline.rs`
- `tests/smoke_cli.rs`
- `tests/fixtures/phase7/*`

## Milestones

- [x] Add the phase-7 exec plan, scenario fixture roots, and scenario-aware snapshot artifact plumbing.
- [x] Harden state continuity, breadcrumbs, and uncertainty/missing-capability copy while keeping renderers pure.
- [x] Add scenario regression coverage across compact and wide layouts, then run the full verification and smoke commands.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-ui-snapshots`
- `cargo run -- ui snapshot --fixture-dir tests/fixtures/phase7 --screen dashboard --screen explain --screen review --screen status --size compact --size wide --out-dir /tmp/ringmaster-ui-snapshots-phase7-smoke`

## Follow-up work

- Consider extending the scenario matrix to `medium` viewport regression tests if compact and wide coverage still misses meaningful layout failures.
- Consider richer scenario docs or a fixture linter if snapshot fixtures become a long-term authoring surface.
