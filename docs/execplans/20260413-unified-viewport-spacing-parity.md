# Unified Viewport Spacing + Demo Parity

## Goal

Unify the viewport contract used by interactive, snapshot, and non-TTY renders, then rebalance dashboard and trends spacing so body content wins earlier than labels or secondary copy.

## Why

The TUI currently feels inconsistent mainly because non-TTY `demo` and `tui` renders still use a bespoke `100x32` fallback while named snapshots use the shared compact/medium/wide contracts. On top of that, dashboard and trends panels still protect labels a little too aggressively, which leaves visible width unused.

## Current state

- Snapshot sizes are defined in `src/ui/snapshot.rs`.
- Breakpoint and metrics logic live in `src/ui/layout.rs`.
- Non-TTY `demo` and `tui` rendering in `src/lib.rs` still hard-code `100x32`.
- Dashboard and trends spacing are tuned independently and still leave some width on the table for labels, badges, and secondary text.

## Desired state

- One shared named viewport contract drives snapshot rendering and non-TTY fallback rendering.
- Non-TTY `demo` and `tui` default to the shared medium viewport.
- Dashboard and trends panels use the same spacing vocabulary while truncating labels earlier to preserve body/chart width.
- Demo and live renders share the same geometry at the same viewport size.

## Constraints

- No CLI flag changes.
- Keep the existing screen list, interaction model, and visual direction.
- Preserve compileable snapshots and truthful live/demo content differences.
- Layer changes carefully on top of existing in-flight edits without reverting them.

## Risks

- Width rebudgeting can destabilize snapshots in compact layouts if truncation gets too aggressive.
- Shared viewport refactors can accidentally desync breakpoint expectations or test fixtures if dimensions are copied instead of reused.

## File plan

- `src/lib.rs`
- `src/tui.rs`
- `src/ui/layout.rs`
- `src/ui/snapshot.rs`
- `src/ui/chrome.rs`
- `src/ui/telemetry.rs`
- `src/components/dashboard.rs`
- `src/components/trends.rs`
- `docs/execplans/20260413-unified-viewport-spacing-parity.md`

## Milestones

- [x] Add a shared viewport preset helper and move non-TTY fallback rendering onto the shared medium contract.
- [x] Rebudget shared shell/title spacing plus dashboard panel fit so chart and matrix bodies retain more width.
- [x] Rebalance trends matrix width allocation, update regression coverage, and run verification.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

## Follow-up work

- If we later need user-selectable non-TTY sizes, add CLI/config surface on top of the shared viewport contract rather than reintroducing bespoke dimensions.
- Revisit compact-only copy elision if future panels add more secondary annotations.

## Completion notes

- Non-interactive `demo` and `tui` now render with the shared `medium` viewport contract instead of a bespoke fallback size.
- Medium layout metrics now spend less horizontal space on panel padding and badge width than wide layouts.
- Weekly heatmap labels now clamp to the reserved column and abbreviate before stealing grid width, while the trends matrix and readiness breakdown now truncate labels earlier so bars and spark signatures can use the reclaimed space.
