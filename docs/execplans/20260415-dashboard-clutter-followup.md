# Dashboard Clutter Follow-up

## Goal

Finish the remaining dashboard clutter cleanup after the overlay-first instrument pass by tightening the readiness-breakdown rows, giving respiratory rate a stronger in-card signal, and reducing weekly-trends row noise.

## Why

`review.md` still calls out clutter, weak small-panel signal density, and weekly-trends competition. The larger dashboard contract is now in place, so this pass focuses on the remaining visual debt instead of reopening the whole redesign.

## Current state

- The dashboard already uses overlay-first drill-down, compact footer inspection, and shared panel scaffolds.
- Readiness breakdown rows still spend extra vertical space and feel table-like instead of instrument-like.
- `Resp Rate` still underuses its tile in tighter layouts.
- Weekly trends still introduces more row spacing than the content needs in roomy viewports.

## Desired state

- Breakdown rows read as tight aligned comparison rails with less blank air.
- `Resp Rate` shows compact recent movement and comparison context even when the panel is shallow.
- Weekly trends remains a 7-day heatmap but wastes less vertical space and feels quieter.

## Constraints

- Keep the existing dashboard interaction model intact.
- Preserve the measured layout helpers rather than hand-placing rows in render code.
- Keep rendering pure and update snapshots/tests with any intentional output change.

## Risks

- Tightening row density could make the bottom band feel cramped if the support lane is not preserved.
- Snapshot churn is expected because the dashboard text layout is changing again.

## File plan

- `src/components/dashboard.rs`
- `src/ui/layout.rs`
- `src/tui.rs`
- `docs/execplans/20260415-dashboard-clutter-followup.md`

## Milestones

- [x] tighten breakdown row density and labeling
- [x] improve respiratory-rate small-panel signal density
- [x] reduce weekly-trends row noise and refresh snapshots/tests

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- ui snapshot --demo --screen dashboard --size compact --size medium --size wide --out-dir /tmp/rm-dashboard-clutter-followup`
- `cargo run -- doctor`

## Follow-up work

- If the dashboard stays stable after this pass, extract the remaining panel-specific compact render decisions into shared small-panel helpers instead of keeping title-specific branches in `dashboard.rs`.
