# Shared Shell And Rich Weekly Trends

## Goal

Extend the authored panel shell into the `Timeline`, `Trends`, and `Ops` screens, and add a richer wide-layout dashboard weekly trends mode that can use more than seven days of history.

## Why

The dashboard now has a disciplined shell/title-row system, but the rest of the app still looks like an older chrome language. The weekly trends panel also underuses wide layouts when richer history exists.

## Current state

- `Dashboard` uses the shared shell and normalized spacing metrics.
- `Timeline`, `Trends`, and `Ops` still mostly render through `panel_block(...)`.
- Dashboard weekly trends only renders a single 7-day slice, regardless of viewport width or available history.

## Desired state

- `Timeline`, `Trends`, and `Ops` use the same shell/title-row/focus treatment as `Dashboard`.
- Wide dashboard layouts render a denser weekly heatmap when more than seven days of history are available.
- Compact and medium weekly heatmaps keep the simpler recent view.

## Constraints

- No IA, navigation, or hybrid-`Enter` changes.
- No new widgets or data families.
- No regression in snapshotability or keyboard behavior.
- Keep monochrome-safe semantics and deterministic render output.

## Risks

- Badge widths are normalized, so long status labels must be shortened carefully to avoid ragged title rows.
- Weekly heatmap selection is driven from selected day state, so richer history mode must not desync the inspector or selection summary.
- Shell rollout across three screens touches a lot of render code and can easily shift snapshots more than intended.

## File plan

- `src/app.rs`
- `src/components/dashboard.rs`
- `src/components/timeline.rs`
- `src/components/trends.rs`
- `src/components/ops.rs`
- `src/ui/telemetry.rs`
- `src/tui.rs`
- `docs/execplans/20260413-shared-shell-and-rich-weekly-trends.md`

## Milestones

- [x] add weekly heatmap recent/history model support
- [x] update dashboard heatmap rendering and inspector selection handling
- [x] roll the shared shell into timeline, trends, and ops
- [x] refresh snapshots/tests and run verification

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --screen timeline --screen trends --screen status --size compact --size wide --out-dir /tmp/ringmaster-followup-shell`
- `cargo run -- ui snapshot --fixture-dir tests/fixtures/phase7 --screen dashboard --size wide --out-dir /tmp/ringmaster-followup-dashboard`

## Follow-up work

- Extend the shell rollout to `Explain`, `Patterns`, `Review`, and `AI`.
- Consider a richer matrix/body typography pass for `Trends` once shell unification is complete.
