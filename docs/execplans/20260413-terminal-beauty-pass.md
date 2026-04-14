# Terminal Beauty Pass

## Goal

Make the dashboard feel like a premium terminal monitoring instrument through sharper tokens, calmer border hierarchy, stronger telemetry compositions, and tighter footer/readout polish without changing interaction or information architecture.

## Why

The current dashboard structure is correct, but the visual result is still flat: hero tiles are under-art-directed, focus and freshness compete, the physiology row feels repetitive, weekly trends underuse their panel, and readiness breakdown still reads as a slab instead of an instrument.

## Current state

- `src/ui/theme.rs` still maps a small palette onto many semantic jobs.
- `src/ui/chrome.rs` gives focused panels a thicker border, but the overall shell hierarchy still leans too heavily on repeated rectangles and title separators.
- `src/components/dashboard.rs` has the right panels and data, but several bodies are still mostly text plus whitespace.
- Existing dashboard snapshot coverage proves breakpoint/layout behavior, but the current text artifacts still look sparse in the hero row and bottom-right panels.

## Desired state

- Focus and freshness are visually distinct.
- Dashboard panel spacing, title bars, badges, and separators follow one authored rhythm.
- Readiness, Sleep, and Activity feel balanced, dense, and data-first.
- The middle row reads as one telemetry family with measurement-specific encoded forms.
- Weekly Trends and Readiness Breakdown use their footprints intentionally.
- The footer reads like a concise instrument status line instead of a hotkey billboard.

## Constraints

- Do not reopen dashboard IA, screen architecture, focus order, or the keyboard model.
- Do not add screens, new persistent data, or decorative filler visuals.
- Keep honest stale/missing/error states explicit and deterministic.
- Keep rendering pure and snapshotable.

## Risks

- Shared token and shell changes can ripple into non-dashboard screens that reuse the same helpers.
- Over-tightening padding can hurt compact layouts if not validated across all three breakpoints.
- Weekly trends and breakdown changes must stay aligned with the existing selected-day and focused-footer behavior.

## File plan

- `src/ui/theme.rs`
- `src/ui/layout.rs`
- `src/ui/chrome.rs`
- `src/ui/telemetry.rs`
- `src/components/dashboard.rs`
- `src/app.rs`
- `src/tui.rs`
- `docs/DESIGN_SYSTEM.md`
- `docs/execplans/20260413-terminal-beauty-pass.md`

## Milestones

- [x] Add the beauty-pass ExecPlan and refresh the dashboard token/shell vocabulary.
- [x] Rework dashboard hero, middle-row, weekly-trends, breakdown, and footer compositions.
- [x] Refresh docs, snapshot assertions, and full verification.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --screen dashboard --size compact --size medium --size wide --out-dir /tmp/ringmaster-terminal-beauty-pass`
- `cargo run -- ui snapshot --fixture-dir tests/fixtures/phase7 --screen dashboard --size compact --size medium --size wide --out-dir /tmp/ringmaster-terminal-beauty-pass-fixtures`

## Follow-up work

- Consider a lighter-weight cross-screen chrome pass for non-dashboard surfaces once the beauty pass settles.
- Revisit whether other telemetry-first screens should inherit any dashboard-only instrument helpers after this pass proves out.

## Verification notes

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --screen dashboard --size compact --size medium --size wide --out-dir /tmp/ringmaster-terminal-beauty-pass`
- `cargo run -- ui snapshot --fixture-dir tests/fixtures/phase7 --screen dashboard --size compact --size medium --size wide --out-dir /tmp/ringmaster-terminal-beauty-pass-fixtures`
