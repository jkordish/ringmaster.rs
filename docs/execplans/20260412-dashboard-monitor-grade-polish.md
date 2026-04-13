# Dashboard Monitor-Grade Polish Pass

## Goal

Polish the redesigned dashboard so it feels like an authored terminal instrument cluster rather than a functional first pass.

## Why

The current dashboard has the right information architecture and focus model, but the visual execution is still uneven: too much top chrome, inconsistent title rows and keylines, prose-heavy overview tiles, and underused bottom-row space.

## Current state

- `src/tui.rs` still spends significant vertical budget on stacked global chrome.
- `src/ui/chrome.rs` and `src/ui/telemetry.rs` provide low-level helpers, but dashboard renderers still make ad hoc spacing and title decisions.
- `src/components/dashboard.rs` renders the right panel set, but several panels still rely on paragraph-style bodies and inconsistent visual anchors.
- Snapshot coverage exists for dashboard widths and degraded scenarios, but not for the focused visual states this pass is targeting.

## Desired state

- The top of the screen is slimmer and calmer.
- Dashboard panels share one shell contract for title baseline, badge placement, padding, and focus treatment.
- Overview tiles are visual-first and prose-last.
- Readiness Breakdown and Weekly Trends use their space intentionally.
- The footer becomes a terse one-line inspector rather than a mini cheat sheet.

## Constraints

- Do not reopen dashboard IA, focus order, or hybrid `Enter` behavior.
- Do not rewrite the data model unless a tiny render-oriented helper is unavoidable.
- Keep monochrome-safe semantics and deterministic snapshots.
- Keep the dashboard terminal-native, dense, and explicit about stale or missing data.

## Risks

- Shared shell changes can ripple into multiple screens if helper behavior changes too broadly.
- Slimming global chrome without harming readability requires careful snapshot review across compact, medium, and wide breakpoints.
- Removing prose from overview mode must preserve honest stale/missing-state communication.

## File plan

- `src/tui.rs`
- `src/ui/layout.rs`
- `src/ui/chrome.rs`
- `src/ui/telemetry.rs`
- `src/components/dashboard.rs`
- `src/app.rs`
- related dashboard snapshot tests in `src/tui.rs`

## Milestones

- [x] Add shared dashboard spacing metrics and a reusable panel shell/title-row helper.
- [x] Slim the top app chrome and tighten the footer inspector.
- [x] Refactor dashboard panels onto the new shell and visual-first overview rendering.
- [x] Refresh dashboard snapshot coverage for widths, focus states, and degraded states.
- [x] Run formatting, lint, tests, doctor, and snapshot smoke validation.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --screen dashboard --size compact --size medium --size wide --out-dir /tmp/ringmaster-dashboard-polish`
- `cargo run -- ui snapshot --fixture-dir tests/fixtures/phase7 --screen dashboard --size compact --size medium --size wide --out-dir /tmp/ringmaster-dashboard-polish-scenarios`

## Follow-up work

- Consider extending the new shell/title-row helper to Timeline, Trends, and Ops once the dashboard polish settles.
- Revisit whether the app-wide top chrome should eventually adopt a lighter-weight divider treatment outside the dashboard pass.
