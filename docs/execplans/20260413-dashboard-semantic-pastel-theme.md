# Dashboard Semantic Pastel Theme

## Goal

Add a restrained semantic pastel color system to the dashboard, with graceful terminal fallbacks, clearer separation between focus/freshness/health meanings, and color-aware snapshot QA.

## Why

The dashboard structure is already strong, but the current presentation is too visually flat. This pass improves scanability and polish without changing screen structure, navigation, or keyboard behavior.

## Current state

- The theme uses a small RGB-only tone set with overloaded semantics.
- Dashboard internals render mostly as monochrome paragraph text.
- Focus, freshness, and judged state can bleed together through shared tone usage.
- Snapshots capture structure only and do not preserve color intent.

## Desired state

- Explicit semantic tokens for interaction, freshness, judged status, raw deltas, and unavailable states.
- Truecolor, 256-color, 16-color, and monochrome-safe fallbacks.
- Dashboard panels that stay neutral overall but use sparse color where it adds meaning.
- Weekly trends use an ordered ramp rather than stoplight coloring.
- Snapshot QA includes ANSI sidecars while plain-text snapshots remain stable.

## Constraints

- Do not reopen architecture.
- Do not add new screens or change keyboard behavior.
- Keep compact, medium, and wide layouts intact.
- Preserve monochrome safety and avoid color-only meaning.
- Keep UI rendering pure and local-first.

## Risks

- Theme refactors can unintentionally affect non-dashboard screens.
- Styling richer dashboard spans can disturb snapshot layouts if spacing shifts.
- ANSI sidecar support adds CLI and snapshot-surface complexity.

## File plan

- `docs/execplans/20260413-dashboard-semantic-pastel-theme.md`
- `src/ui/theme.rs`
- `src/ui/chrome.rs`
- `src/ui/charts.rs`
- `src/ui/telemetry.rs`
- `src/components/dashboard.rs`
- `src/app.rs`
- `src/tui.rs`
- `src/ui/snapshot.rs`
- `src/cli.rs`
- `src/lib.rs`
- `tests/smoke_cli.rs`
- `docs/DESIGN_SYSTEM.md`
- `README.md`

## Milestones

- [x] Add semantic theme tokens and terminal capability fallbacks.
- [x] Apply restrained semantic color rendering across dashboard panels.
- [x] Add ANSI snapshot sidecars, update docs, and verify with tests.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --screen dashboard --size compact --size medium --size wide --ansi-sidecar --color-mode truecolor --color-mode mono`

## Follow-up work

- Consider extending semantic presentation helpers to non-dashboard screens if the shared theme refactor reveals obvious wins.
- Revisit SpO2 categorical styling if richer category data becomes available from the Oura model layer later.
