# Phase 6 Visual Language And Interaction Choreography

## Goal

Turn the existing Ratatui product into a coherent visual instrument by introducing a centralized design system, stronger screen-specific reading paths, deterministic visual QA commands, and regression coverage without widening the Oura data surface.

## Why

Phase 5 delivered a useful product, but the current UI still reads like a stable internal tool rather than a deliberately designed observability instrument. Most screens share the same box-and-list language, hierarchy is flatter than it should be, and there is no single source of truth for color, spacing, chrome, or snapshot-driven visual review.

## Current state

- The TUI is stable and pure, with all screens driven from persisted `LiveSnapshot` state.
- Styling is mostly ad hoc in component renderers, with repeated `Block::default().borders(Borders::ALL)` usage and only a few direct hardcoded colors.
- Snapshot rendering exists as a deterministic string buffer path, but there is no dedicated `ui snapshot` command or golden review workflow.
- The major screens already exist, but they do not yet have strongly differentiated visual roles.

## Desired state

- `docs/DESIGN_AUDIT.md` explicitly documents current visual debt and hierarchy problems.
- `docs/DESIGN_SYSTEM.md` defines the visual language and the codebase has centralized theme/token modules to match.
- Each primary screen has a distinct reading path and visual role while preserving the current state architecture.
- `ringmaster ui snapshot --demo --out-dir <dir>` generates deterministic text snapshots for multiple screens and sizes.
- Visual regression coverage exists for core screens, breakpoints, and state semantics.

## Constraints

- Local-first and privacy-first only.
- Single-package crate unless implementation pressure proves otherwise.
- Pure Ratatui components: no HTTP, token refresh, or database writes in screen renderers.
- Reuse the current `Event -> Action -> State -> Render` flow.
- Keep information density useful; avoid sparse redesign theater.
- No `unwrap`, `expect`, `todo!`, `panic!`, or `dbg!` in non-test code.
- No state meaning that depends only on color.

## Risks

- A purely text snapshot path cannot capture color by itself, so state-style testing must also validate semantic token output directly.
- Stronger visual roles may require modest presentation-model changes; these need to stay narrow so `app.rs` remains the sole shaping layer.
- Overcorrecting away from “box soup” could make some screens ambiguous in cramped terminals if breakpoint rules are weak.

## File plan

- `docs/execplans/20260409-phase6-visual-language-and-interaction-choreography.md`
- `docs/DESIGN_AUDIT.md`
- `docs/DESIGN_SYSTEM.md`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`
- `docs/IMPLEMENT.md`
- `src/cli.rs`
- `src/lib.rs`
- `src/tui.rs`
- `src/app.rs`
- `src/components/*`
- `src/ui/*`
- `tests/smoke_cli.rs`
- snapshot-oriented tests in `src/tui.rs` and `src/ui/snapshot.rs`

## Milestones

- [x] Milestone 1: create the phase-6 exec plan, design audit, design-system docs, and initial UI module seams.
- [x] Milestone 2: add centralized theme/layout/chrome helpers and refactor the shared app frame to use them.
- [x] Milestone 3: redesign Dashboard, Timeline, Trends, Explain, Patterns, Review, and Status around distinct visual roles and breakpoint-aware layouts.
- [x] Milestone 4: add the `ui snapshot` command, deterministic artifact generation, and snapshot/state regression tests.
- [x] Milestone 5: update product docs, run the full validation sweep, and repair any failures.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-ui-snapshots`
- additional bounded visual smoke checks covering Dashboard, Timeline, Review, and Status across at least compact and wide terminal sizes

Completed on `2026-04-09`:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-ui-snapshots`
- `cargo run -- ui snapshot --demo --screen dashboard --screen timeline --screen review --screen status --size compact --size wide --out-dir /tmp/ringmaster-ui-snapshots-smoke`

## Follow-up work

- PNG/image-based snapshot export remains deferred; text snapshots are the canonical QA surface for now.
- Packaging, installers, release automation, notifications, and new analytics families remain outside this pass.
- Any richer terminal animation or transitions remain deferred unless a later pass can justify them without noise.
