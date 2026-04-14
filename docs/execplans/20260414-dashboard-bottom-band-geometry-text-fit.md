# Dashboard Bottom-Band Geometry And Text-Fit Pass

## Goal

Make the dashboard bottom band feel mathematically composed by giving `Readiness Breakdown` and `Weekly Trends` explicit internal geometry, deterministic text-fit rules, and shared spacing primitives.

## Why

The current dashboard visual direction is good enough, but the bottom band still feels hand-placed. Labels, chart bodies, legends, and support copy compete for space, and truncation still happens opportunistically instead of through a deliberate fit policy.

## Current state

- `src/components/dashboard.rs` computes bottom-band widths and line content inside the renderers.
- Weekly Trends still mixes geometry concerns across layout helpers and renderer-local line building.
- Readiness Breakdown combines chart body, band strip, and focus narrative in one shared text block.
- Generic text-fit helpers live in `src/ui/telemetry.rs`, which mixes fit policy with chart glyph utilities.

## Desired state

- Both bottom panels use explicit named zones: shell title row, chart body, and dedicated support/summary lanes.
- Shared chart metrics and layout structs drive all bottom-band geometry.
- Weekly Trends derives headers, grid columns, selection markers, legend origin, and summary origin from one layout function.
- Readiness Breakdown derives label, signal, and delta columns from one layout function and keeps support text out of the chart body.
- Dense chart text truncates intentionally and deterministically through shared helpers.

## Constraints

- Do not reopen palette, visual direction, screens, keyboard model, or overall dashboard IA.
- Keep each bottom panel a single composite focus region.
- Keep rendering pure and deterministic for snapshots.
- Ship compileable steps throughout the refactor.

## Risks

- Moving layout primitives may ripple into tests that currently assume renderer-local spacing.
- Medium-width dashboards have just enough height for the new lanes; lane math must degrade cleanly without overlap.
- Text-fit tightening can accidentally hide useful detail unless footer/inspector behavior stays intact.

## File plan

- `docs/execplans/20260414-dashboard-bottom-band-geometry-text-fit.md`
- `src/ui/mod.rs`
- `src/ui/layout.rs`
- `src/ui/telemetry.rs`
- `src/ui/text_fit.rs`
- `src/components/dashboard.rs`
- `src/tui.rs`

## Milestones

- [x] Add the ExecPlan plus shared chart-metric and text-fit helpers.
- [x] Refactor `Readiness Breakdown` onto explicit body/support geometry and fixed columns.
- [x] Refactor `Weekly Trends` onto explicit header/grid/legend/summary geometry from one layout model.
- [x] Add regression coverage for layout alignment, text-fit determinism, and dashboard bottom-band snapshots.
- [x] Run formatting, tests, snapshots, and doctor verification; record any dense-width compromises.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --screen dashboard --size medium --size wide --out-dir /tmp/ringmaster-bottom-band`
- `cargo run -- ui snapshot --fixture-dir tests/fixtures/phase7 --screen dashboard --size medium --size wide --out-dir /tmp/ringmaster-bottom-band-fixtures`

## Follow-up work

- Revisit compact dashboard bottom-band density only if this pass exposes a real regression there.
- Consider lifting the new text-fit helpers into other telemetry-dense screens only after this dashboard-only pass settles.

## Result

- The bottom band now renders from shared geometry primitives instead of renderer-local offsets.
- `Readiness Breakdown` uses fixed label, signal, and delta columns with a dedicated support lane.
- `Weekly Trends` uses explicit header, grid, legend, and summary zones driven from one layout model.
- Medium and dense-width layouts still abbreviate a few labels intentionally to preserve chart real estate, but truncation is now deterministic and support copy no longer compresses the chart body.
