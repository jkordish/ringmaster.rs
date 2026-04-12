# Monitoring Dashboard Telemetry Redesign

## Goal

Redesign the Ratatui experience so `ringmaster.rs` reads like a local-first health telemetry console instead of a prose-first report, with the Dashboard as the primary instrument cluster and Timeline, Trends, and Status aligned to the same interaction model.

## Why

The current dashboard still relies on paragraph summaries and generic cards, which makes it hard to scan. The new monitoring-style layout should surface readiness, sleep, activity, physiology, freshness, and scope coverage at a glance while keeping details available through focus, inspection, and drill-down.

## Current state

- `src/app.rs` builds centralized screen models, but Dashboard, Trends, and Ops models are still string-heavy.
- `src/navigation.rs` uses a coarse region model that is too broad for panel-by-panel focus semantics.
- `src/components/dashboard.rs`, `trends.rs`, and `ops.rs` render list/card/prose-heavy layouts.
- Snapshot coverage exists, but it does not yet cover missing-scope or rate-limited telemetry states.

## Desired state

- Dashboard uses a stable telemetry grid aligned to the mockup silhouette.
- A shared telemetry widget layer provides rings, rails, sparklines, histograms, heatmaps, status badges, and a footer inspector.
- Focus is panel-specific, inspector-first, and consistent across Dashboard, Timeline, Trends, and Status.
- Missing scopes, stale data, unsupported metrics, rate limits, and other degraded states render as stable panel shells instead of disappearing.

## Constraints

- Keep the application local-first and privacy-first.
- Preserve pure UI rendering boundaries.
- Do not fabricate unavailable Oura metrics.
- Keep existing viewport breakpoints unless a layout bug forces a targeted change.
- Ship compileable changes with tests and updated snapshots.

## Risks

- The centralized model and navigation code may make the refactor broad and easy to destabilize.
- Some requested metrics, especially HRV and respiratory rate, are not fully wired into the current live snapshot and need honest placeholder states.
- Snapshot expectations and navigation tests will need coordinated updates.

## File plan

- `src/app.rs`
- `src/navigation.rs`
- `src/tui.rs`
- `src/components/dashboard.rs`
- `src/components/timeline.rs`
- `src/components/trends.rs`
- `src/components/ops.rs`
- `src/ui/mod.rs`
- `src/ui/chrome.rs`
- `src/ui/snapshot.rs`
- `src/ui/telemetry/*`
- `src/store/queries.rs` consumer paths in `src/app.rs`
- `src/lib.rs` scenario overlays
- related tests and snapshots

## Milestones

- [x] Add the telemetry widget layer and panel-frame helpers.
- [x] Refactor dashboard and snapshot/live models to carry typed telemetry state.
- [x] Replace coarse dashboard focus with panel-specific focus and footer inspection.
- [x] Align Timeline, Trends, and Status to the shared widget language and updated navigation model.
- [x] Add scenario coverage for missing-scope and rate-limited states, then refresh snapshots and tests.

## Completed notes

- Dashboard now renders as a dense monitoring grid with stable panel shells for readiness, sleep, activity, physiology, readiness drivers, and weekly trends.
- Timeline, Trends, and Status reuse the shared telemetry framing and focus semantics instead of the older prose-first card patterns.
- The footer inspector is now live and focus-bound across the redesigned monitoring surfaces.
- Scenario fixture snapshots now cover `stale`, `error`, `missing-scope`, and `rate-limited` states across compact, medium, and wide layouts.
- HRV and respiratory-rate remain explicit `Unsupported` or `NoData` shells because the current live snapshot still does not carry trustworthy series for those metrics.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- existing snapshot or UI regression commands

## Follow-up work

- Wire true HRV and respiratory-rate visualizations once the live snapshot carries those series.
- Consider moving more screen model types out of `src/app.rs` after the telemetry system stabilizes.
