# Dashboard Bottom-Band Packing And Grouped Labels

## Goal

Finish the dashboard bottom-band geometry work by replacing the remaining top-clamped internal layouts in `Readiness Breakdown` and `Weekly Trends` with deliberate packed chart clusters, grouped labels, and deterministic text-fit behavior.

## Why

The measured layout pass fixed column alignment, but both bottom panels still leave too much unused vertical space inside their already-assigned panel rectangles. The result still feels under-packed, especially in medium and wide dashboard layouts.

## Current state

- `src/ui/layout.rs` separates chart/support zones but still top-anchors the bottom-band chart clusters.
- `Readiness Breakdown` has aligned columns, but its factor rows, band strip, and support lane do not feel composed as one packed instrument.
- `Weekly Trends` still duplicates labels across repeated subrows and does not use grouped metric labels.
- Text-fit already uses deterministic helpers, but the weekly grouped-label pass and remaining bottom-band support-copy tightening are not implemented yet.

## Desired state

- Both bottom panels are composed as `title row -> packed chart cluster -> one-line support/summary lane`.
- Extra vertical space is distributed intentionally through padding and internal group/row gaps instead of remaining as dead space.
- `Readiness Breakdown` behaves like a packed chart-table with explicit compact labels and a band strip aligned to the signal viewport.
- `Weekly Trends` renders 3 grouped metric labels (`Sleep`, `Ready`, `Actv`) over 2 subrows each, with one geometry source for headers, cells, legend, summary, and selection markers.

## Constraints

- Keep the existing outer dashboard pane constraints unchanged in compact, medium, and wide.
- Keep the shared shell/title/badge system unchanged.
- Keep the semantic pastel palette unchanged.
- Keep one-line in-panel support lanes as the default.
- Keep `Readiness Breakdown` and `Weekly Trends` as composite dashboard focus regions.
- Do not add new data families or change the overall keyboard model.

## Risks

- The grouped-label weekly layout needs to stay truthful to the existing selected-day interaction without inventing a new focus target or selection model.
- Medium-width layouts have less surplus height than wide layouts, so the packing logic must degrade cleanly without overlapping rows or losing the summary lane.
- Snapshot churn will be concentrated in the bottom band, so tests need to assert geometry and text-fit intentionally rather than relying only on generic string presence.

## File plan

- `docs/execplans/20260414-dashboard-bottom-band-packing-grouped-labels.md`
- `src/ui/layout.rs`
- `src/ui/text_fit.rs`
- `src/components/dashboard.rs`
- `src/tui.rs`

## Milestones

- [x] Add a shared chart-cluster vertical packing helper for bottom-band panels.
- [x] Refactor `Readiness Breakdown` onto packed cluster geometry with explicit compact label policy.
- [x] Refactor `Weekly Trends` onto grouped labels and packed cluster geometry.
- [x] Add geometry/text-fit assertions and refresh dashboard bottom-band snapshot coverage.
- [x] Run formatting, clippy, tests, doctor, and focused dashboard snapshot verification.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --screen dashboard --size medium --size wide --out-dir /tmp/ringmaster-bottom-band-packing`
- `cargo run -- ui snapshot --fixture-dir tests/fixtures/phase7 --screen dashboard --size medium --size wide --out-dir /tmp/ringmaster-bottom-band-packing-fixtures`

## Follow-up work

- Revisit compact-width and constrained-height weekly density only if we want to preserve a full dedicated summary lane in very short panel bodies; the current medium fallback keeps grouped labels and aligned selection by compressing each metric pair into one physical row before overflowing detail to the footer.
- Consider reusing the new cluster-packing helper in other dense telemetry panels only after this dashboard-only pass settles.
