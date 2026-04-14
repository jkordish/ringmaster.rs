# Measured Internal Layout Implementation Pass

## Goal

Implement a measurement-driven internal layout system for dense dashboard panels and overlays so the dashboard bottom band and help modal feel derived instead of nudged.

## Why

The current UI direction is solid, but a few dense surfaces still feel hand-placed. Bottom-band labels, chart bodies, and support copy compete for the same space, and the help overlay is centered without a true content-fit layout model.

## Current state

- `src/ui/layout.rs` already contains first-pass geometry helpers for `Readiness Breakdown`, `Weekly Trends`, and a simple centered modal bounds helper.
- `src/ui/text_fit.rs` already contains deterministic truncation and a few label-specific fit helpers.
- `src/components/dashboard.rs` still owns too much panel-local fit policy and lets several nearby panels append support text directly into chart bodies.
- `src/tui.rs` renders help as a centered shell, but not from a richer content-fit overlay layout.

## Desired state

- Outer dashboard pane constraints remain unchanged for `draw_compact`, `draw_medium`, and `draw_wide`.
- Internal panel geometry is measurement-driven through shared helpers only.
- Dense panel support text uses a reserved one-line support lane or the footer/inspector, never chart rows.
- `Readiness Breakdown` and `Weekly Trends` derive all internal placement from one shared geometry source each.
- The help overlay uses a centered, bounded, content-fit modal layout and scrolls internally when content exceeds the visible body.

## Constraints

- Do not change outer dashboard pane constraints.
- Do not use Ratatui unstable rendered-line-info APIs.
- Help overflow must scroll, not paginate.
- Dense in-panel support text stays one line by default; fuller context belongs in the footer/inspector.
- No new dependency is required.
- Search and AI preflight keep current behavior unless reusing the richer overlay helper is a strictly mechanical no-behavior-change cleanup.
- Measurement is only for internal panel geometry and overlay sizing/placement, never for outer pane reflow.

## Risks

- Tightening fit rules could change existing snapshots more than expected.
- Reserving stable support lanes in nearby panels could over-compress tiny bodies if minimum row budgets are not handled carefully.
- Help overlay sizing needs to stay stable across compact, medium, and wide viewports without drifting into a larger behavioral change.

## File plan

- `docs/execplans/20260414-measured-internal-layout-pass.md`
- `src/ui/text_fit.rs`
- `src/ui/layout.rs`
- `src/components/dashboard.rs`
- `src/tui.rs`
- `src/app.rs`
- `docs/ARCHITECTURE.md`

## Milestones

- [x] Extend shared text-fit and measurement primitives for dense one-line surfaces.
- [x] Extend shared layout primitives for measured panel support lanes and content-fit overlays.
- [x] Refactor dashboard bottom-band renderers and nearby dense panels onto the shared measured layout helpers.
- [x] Refactor help overlay onto the bounded centered overlay layout helper without changing modal behavior.
- [x] Add regression tests/snapshots and run formatting, clippy, tests, and doctor.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --screen dashboard --size medium --size wide --out-dir /tmp/ringmaster-measured-layout-dashboard`
- `cargo run -- ui snapshot --demo --screen dashboard --size compact --size medium --size wide --out-dir /tmp/ringmaster-measured-layout-dashboard-all`

## Status

Completed on 2026-04-14.

Notes:
- Outer dashboard pane constraints remained unchanged.
- Ratatui unstable rendered-line-info was intentionally avoided.
- Help overlay sizing is now content-fit and centered, with scroll preserved inside the modal.
- Dense support copy now uses measured one-line support lanes and falls back to footer/inspector detail instead of consuming chart rows.

## Follow-up work

- Consider reusing the richer overlay helper for search and AI preflight only after this pass settles and only if it remains a pure cleanup.
- Revisit compact-width dashboard density only if this pass exposes a real regression there.
