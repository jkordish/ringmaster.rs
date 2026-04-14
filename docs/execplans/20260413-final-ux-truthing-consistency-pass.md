# Final UX Truthing + Consistency Pass

## Goal

Make the terminal UI feel dependable and internally consistent without changing the established visual direction, palette, screen list, or panel-based layout.

## Why

The remaining quality gaps are no longer about style; they are about trust. A few focused regions still imply activation when `Enter` does not perform a real visible action, modal overlays still use ad hoc geometry and fuzzy focus, weekly trends still mixes layout math across helpers, and several metric panels badge themselves as `NO DATA` while still showing meaningful baseline or historical context.

## Current state

- Footer hints are derived from static binding scopes, so runtime `Enter` availability can drift from the actual focused item behavior.
- Help, Search, and AI preflight each render and manage transient focus slightly differently.
- Weekly Trends geometry is computed in more than one place, with hand offsets still affecting labels, cells, and legend alignment.
- Metric panels still rely on a coarse telemetry availability badge model that collapses several semantically different states into `NO DATA`.

## Desired state

- Every focusable region resolves to an explicit runtime action contract: navigate, expand, toggle, open overlay, activate command, or none.
- Footer hints reflect the runtime action contract instead of static region bindings.
- Help behaves as a real modal with one shared overlay layout helper, bounded content, trapped focus, and correct focus restore.
- Weekly Trends uses a single layout function for labels, grid origin, selection markers, and legend placement.
- Metric-panel badges distinguish `FRESH`, `STALE`, `NO CURRENT SAMPLE`, `BASELINE ONLY`, `HISTORICAL ONLY`, `MISSING SCOPE`, `UNAVAILABLE`, `EMPTY`, and `ERROR`.

## Constraints

- Keep the current dashboard-first information architecture, viewport modes, semantic pastel theme, and hybrid drill-down policy.
- Do not add new screens, new decorative affordances, or panel-local keyboard rules.
- Preserve deterministic snapshots and keep the tree compileable throughout the pass.

## File plan

- `src/app.rs`
- `src/focus.rs`
- `src/keybindings.rs`
- `src/navigation.rs`
- `src/tui.rs`
- `src/ui/chrome.rs`
- `src/ui/telemetry.rs`
- `src/components/dashboard.rs`
- `src/components/trends.rs`
- `docs/execplans/20260413-final-ux-truthing-consistency-pass.md`

## Milestones

- [x] Add the final execplan and shared interaction/model primitives for runtime actionability, modal layout, and richer panel states.
- [x] Update focus traversal, overlay behavior, and footer hints to honor runtime actionability and strict modal focus restore.
- [x] Unify Weekly Trends geometry and tighten dashboard/trends spacing using shared layout math.
- [x] Add regression coverage for actionability, help modal behavior, weekly trends alignment, state semantics, and keyboard consistency.
- [x] Run verification and record any remaining intentional inspect-only behavior or spacing compromises.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

## Follow-up work

- If search later grows beyond a single query field, move it onto the same richer modal focus model introduced here.
- Revisit whether non-dashboard telemetry panels should adopt the richer panel-state taxonomy once their bodies carry more baseline and historical narratives.

## Completion notes

- Intentional inspect-only focus targets remain on selector-style composites whose truth is arrow-driven rather than activation-driven: trends sort tabs, timeline window selector, review mode and focus tabs, pattern metric tabs, pattern family tabs, and AI browser tabs.
- The dashboard and trends spacing cleanup stayed inside shared layout math and panel metrics. A small amount of asymmetric width remains where compact viewports must preserve readable labels before maximizing chart width.
