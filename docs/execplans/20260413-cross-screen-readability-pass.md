# Cross-Screen Readability Pass

## Goal

Improve medium-width `Trends` readability and reduce truncation in shared status and `Status/Ops` surfaces without reopening the accepted dashboard tradeoffs from the recent beauty pass.

## Why

The current dashboard-first work intentionally left a few follow-up gaps:

- the global top status strip still uses blunt tail truncation
- compact and medium `Status/Ops` clip high-value diagnostics and family rows mid-token
- medium `Trends` still uses the older dense one-line matrix instead of the newer authored typographic treatment

## Current state

- `src/tui.rs` truncates the app status strip with a generic `truncate_line(...)` helper.
- `src/components/ops.rs` renders compact diagnostics, warnings, and family status with line-oriented layouts that clip long values in narrow columns.
- `src/components/trends.rs` uses the legacy one-line matrix in medium layouts and the richer two-line matrix only in wide layouts.

## Desired state

- The top status strip drops low-priority segments before ellipsizing.
- Compact and medium `Status/Ops` preserve key diagnostic values through wrapping or stacked composition instead of mid-token clipping.
- Medium `Trends` adopts a readable two-line row format tuned for `120x36`.

## Constraints

- Do not reopen compact dashboard terseness, the lighter wide `Activity` tile, compact `Timeline` one-line summaries, or the wide-only dense dashboard weekly-history matrix.
- Keep UI rendering pure and deterministic.
- Avoid app-model churn unless a tiny render-only helper is clearly justified.

## Risks

- Shared status-strip logic affects every screen.
- Making `Status/Ops` more readable can reduce visible row count in narrow shells.
- Medium `Trends` can become too airy if the two-line row contract is not sized carefully.

## File plan

- `src/tui.rs`
- `src/components/ops.rs`
- `src/components/trends.rs`
- `src/ui/telemetry.rs` if a small shared formatting helper is needed
- `src/tui.rs` snapshot tests
- `docs/execplans/20260413-cross-screen-readability-pass.md`

## Milestones

- [x] Replace the top status-strip truncation with priority-aware segment selection.
- [x] Rework compact and medium `Status/Ops` narrow readouts for diagnostic readability.
- [x] Refresh medium `Trends` with a two-line matrix body.
- [x] Update snapshot coverage and run full verification.

## Notes

- Medium `Status/Ops` now prioritizes config and database paths alongside auth, queue, and eval diagnostics instead of clipping them out of view.
- Compact warning assertions were relaxed to verify visible warning content rather than one exact warning label because warning ordering is not the user-facing contract.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --screen trends --screen status --size compact --size medium --out-dir /tmp/ringmaster-cross-screen-readable`

## Follow-up work

- Consider a later non-dashboard chrome pass for additional read-only screens if the status-strip reprioritization proves out.
- Revisit compact/medium Timeline footer and inspector density only if this pass suggests a stronger shared narrow-screen contract.
