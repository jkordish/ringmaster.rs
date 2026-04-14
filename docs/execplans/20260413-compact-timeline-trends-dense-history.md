# Compact Timeline, Wide Trends, and Dense-History Proof

## Goal

Close the remaining polish gaps after the dashboard shell rollout by making compact `Timeline` content-led instead of title-led, refreshing the wide `Trends` matrix body, and adding fixture-backed visual proof for the 14-day dashboard weekly-trends mode.

## Why

The current follow-up polish work left three known compromises:

- compact `Timeline` shells still collapse meaningful body content too aggressively
- wide `Trends` still uses the legacy dense text matrix inside the new shell system
- the 14-day weekly-trends mode exists in code but is not demonstrated in fixture-backed snapshots

## Current state

- `Timeline`, `Trends`, and `Ops` already share the new shell/title-row contract.
- Compact `Timeline` keeps all panels, but multiple compact shells render only titles or nearly-empty bodies.
- Wide `Trends` uses the same single-line matrix row format as medium.
- `phase7` fixture scenarios only seed seven daily rows, so wide dashboard heatmaps never switch into dense-history mode in visual evidence.

## Desired state

- Compact `Timeline` keeps its current panel set and navigation but shows one meaningful body line in each compressed shell.
- Wide `Trends` renders an authored two-line matrix row layout with stable keylines and better use of vertical space.
- A new fixture-backed scenario under `tests/fixtures/phase7` triggers the dense 14-day weekly-trends mode, and snapshot coverage proves it.

## Constraints

- No IA, focus-order, or keyboard-behavior changes.
- No model churn beyond what is needed for rendering and fixture-backed snapshot coverage.
- Keep UI rendering pure and deterministic.
- Preserve the existing `phase7` scenario semantics for `strong`, `weak`, `empty`, `stale`, `error`, `missing-scope`, and `rate-limited`.

## Risks

- Compact `Timeline` can become too terse if truncation is overly aggressive.
- Wide `Trends` can become harder to scan if the two-line row format overuses whitespace or weakens numeric comparison.
- Snapshot scenario expansion touches enum/tests/fixture-root detection in a few places, so it is easy to miss one expectation.

## File plan

- `src/components/timeline.rs`
- `src/components/trends.rs`
- `src/ui/snapshot.rs`
- `src/lib.rs`
- `src/tui.rs`
- `tests/fixtures/phase7/*`
- `docs/execplans/20260413-compact-timeline-trends-dense-history.md`

## Milestones

- [x] rebudget compact `Timeline` shells and replace compact list bodies with concise summary lines
- [x] refresh the wide `Trends` matrix body into a two-line typographic layout
- [x] add a new fixture-backed dense-history scenario and wire it through snapshot scenario plumbing
- [x] update targeted snapshot tests and run full verification

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --screen timeline --screen trends --size compact --size wide --out-dir /tmp/ringmaster-followup-polish`
- `cargo run -- ui snapshot --fixture-dir tests/fixtures/phase7 --screen dashboard --size wide --out-dir /tmp/ringmaster-followup-dense-history`

## Follow-up work

- Revisit medium `Trends` if the wide typography refresh suggests a better compact-medium shared matrix contract.
- Consider a future compact-specific timeline footer that mirrors selected event/detail state more explicitly if the one-line shells still feel too terse.
