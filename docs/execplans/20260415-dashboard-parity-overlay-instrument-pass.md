# Dashboard Parity Overlay Instrument Pass

## Goal

Finish the dashboard parity work and land the overlay-first dashboard interaction model, richer panel instrumentation, and the shared transient plumbing needed to make dashboard detail exploration feel consistent in demo and live mode.

## Status

Completed on 2026-04-15 after landing the presenter parity updates, dashboard-local detail overlays, panel rebalancing, shared footer/help/status refinements, and the required verification sweep.

## Why

`review.md` pushed the dashboard toward a stronger monitor-grade interaction model, but part of that scope overlapped work already shipped in the dashboard shell and part of it depended on unfinished demo/live parity cleanup. This pass folds both into one coherent dashboard contract instead of stacking a redesign on top of mismatched presenter behavior.

## Current state

- Demo and live already shared one dashboard build path, but default day selection still favored the newest day even when that day was too sparse to present the dashboard well.
- Dashboard `Enter` behavior was inconsistent across regions, mixing cross-screen navigation, inline expansion, and activation no-ops.
- The footer/help/keybinding stack did not have a dedicated transient model for dashboard-local detail views.
- Several secondary dashboard panels still rendered with uneven weight and sparse-state behavior.

## Desired state

- Demo and live dashboards use the same presenter-driven panel contract and default to the latest renderable recent day.
- Every dashboard region opens a dashboard-local detail overlay on activation.
- Search, help, dashboard detail, and AI preflight transient layers have a clear priority order and restoration behavior.
- Dashboard panels keep their compact scan lane in-card while moving deeper explanation into the footer and detail overlay.

## Constraints

- Keep the app local-first and preserve the existing screen architecture.
- Do not require new network or storage flows for the dashboard interaction rewrite.
- Do not regress non-dashboard screen activation behavior.
- Keep rendering pure inside `src/components/*`.

## Risks

- Dashboard overlay plumbing touches app navigation, keybindings, and TUI rendering at once, so regression risk is spread across multiple layers.
- Default-day heuristics can subtly change live-mode expectations when the newest day is intentionally sparse.
- Visual rebalancing can unintentionally reduce legibility on compact viewports if not covered by tests.

## File plan

- `src/app.rs`
- `src/components/dashboard.rs`
- `src/navigation.rs`
- `src/keybindings.rs`
- `src/tui.rs`
- `docs/execplans/20260414-demo-live-dashboard-parity-pass.md`
- `docs/execplans/20260415-dashboard-parity-overlay-instrument-pass.md`

## Milestones

- [x] Fold dashboard day selection and presenter parity work into the shared live/demo dashboard path.
- [x] Replace dashboard mixed activation behavior with a dedicated detail overlay transient and shared keybinding/help support.
- [x] Rebalance dashboard panel rendering and add tests for overlay behavior, transient ordering, and default-day selection.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

### Verification result

- Passed on 2026-04-15.

## Follow-up work

- If the overlay-first dashboard pattern proves successful elsewhere, extract a more general reusable “detail overlay” helper instead of keeping the dashboard-specific variant in app state.
- If live usage shows the render-anchor heuristic is too strict or too loose, move it into a separately testable policy helper with fixture-backed tuning cases.
