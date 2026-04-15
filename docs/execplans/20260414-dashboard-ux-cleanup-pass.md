# Dashboard UX Cleanup Pass

## Goal

Tighten the Ratatui dashboard so missing-data states read as intentional, Weekly Trends shows a navigable 7-day window by default, the readiness breakdown no longer contains an unclear waveform band, and dashboard/footer copy stays bounded at typical terminal widths.

## Why

The current dashboard has several cards that look blank, clipped, or half-rendered when current-day Oura data is absent. The result is misleading UX: users see rendering artifacts instead of deliberate stale, baseline-only, or unavailable states.

## Current state

- Dashboard cards derive detailed `MetricPanelState` values, but many renderers collapse non-fresh states into a generic dotted scaffold.
- Weekly Trends renders 7 days in medium widths and 14 days in wide widths, which overloads the panel.
- Readiness Breakdown reserves space for an unlabeled `band` waveform that is not a first-class factor.
- Panel title badges are width-limited and truncate longer labels accidentally.
- Dashboard footers repeat freshness/state concepts already visible in panel badges.

## Desired state

- Dashboard metric tiles render four clear presentation states: fresh, baseline-only, stale, unavailable.
- Missing-data tiles always show a deliberate primary line and concise explanatory secondary line.
- Weekly Trends always shows one 7-day window and pages through history without clipping.
- Readiness Breakdown shows only semantically clear rows.
- Badges and body copy shorten intentionally on narrow widths instead of clipping.
- Footer copy stays concise and avoids repeated freshness language.

## Constraints

- Preserve the existing terminal-native layout and visual tone.
- Keep UI rendering pure; all presentation data must be built in app/model code.
- Avoid changing the broader shared day-selection model unless the dashboard windowing work requires it.
- Ship compileable incremental changes with updated tests and docs.

## Risks

- Weekly windowing could accidentally desynchronize dashboard day selection from other screens if the visible window math is wrong.
- Snapshot churn will be broad because this touches multiple dashboard states and panel titles.
- Over-shortening copy could make tiles feel vague if the compact variants are too aggressive.

## File plan

- `src/app.rs`
- `src/components/dashboard.rs`
- `src/ui/text_fit.rs`
- `src/ui/chrome.rs`
- `src/ui/telemetry.rs`
- `src/keybindings.rs`
- `src/ui/layout.rs`
- `src/tui.rs`
- `docs/KEYBINDINGS.md`
- `docs/STATUS.md`

## Milestones

- [x] Add the dashboard-specific state/copy model and update tile builders.
- [x] Rework dashboard renderers, readiness breakdown, and weekly heatmap navigation/rendering.
- [x] Tighten badge/footer copy, refresh tests/snapshots, and update docs.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --screen dashboard --size compact --size medium --size wide --out-dir /tmp/ringmaster-dashboard-ux`

## Follow-up work

- Consider a future dashboard-specific help legend if the weekly heatmap paging needs stronger on-screen affordance than footer hints provide.
- Revisit whether the dashboard should eventually decouple weekly-history browsing from the shared selected-day state across screens.
