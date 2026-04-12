# Hidden Expert Controls Navigation Consistency Pass

## Goal

Audit the remaining screen-specific shortcut-only workflows and either promote them into visible canonical controls or explicitly demote them to expert aliases.

## Why

Phase 8 established a shared keyboard grammar, but a few high-value actions still depend on memorized letters. This pass closes the gap so every important workflow has a visible canonical path.

## Current state

- Timeline still relies on hidden zoom and overlay toggles.
- Explain still relies on hidden overlay toggles.
- Patterns has a visible metric selector, but family filters remain hidden.
- AI still hides saved-artifact actions behind expert letters and the preflight modal does not render its control row.

## Desired state

- Timeline shows visible window and overlay selectors.
- Explain shows visible overlay selectors.
- Patterns shows both metric and family selectors.
- AI shows a visible artifact-action pane and truthful launch points.
- AI preflight renders its canonical confirm/privacy/cancel controls.
- Remaining letters are documented as expert aliases instead of implied primary workflows.

## Constraints

- Keep UI rendering pure.
- Preserve the shared key model: selectors use horizontal movement, lists use vertical movement, activation uses `Enter` / `Space`.
- Keep `cargo test --all`, `cargo run -- doctor`, and `cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-nav-ui` green after each logical batch.

## Risks

- Region-map changes can break focus restoration and search scope assumptions.
- AI action availability differs by artifact tab, so the visible list must stay truthful.
- Snapshot tests may fail if screen titles or focus labels drift from reducer state.

## File plan

- `src/app.rs`
- `src/navigation.rs`
- `src/keybindings.rs`
- `src/components/timeline.rs`
- `src/components/explain.rs`
- `src/components/patterns.rs`
- `src/components/ai.rs`
- `src/tui.rs`
- `docs/KEYBINDINGS.md`
- `README.md`
- `docs/IMPLEMENT.md`
- `docs/ARCHITECTURE.md`

## Milestones

- [x] Promote visible canonical controls for Timeline, Explain, and Patterns.
- [x] Promote visible canonical artifact actions and preflight controls in AI.
- [x] Update docs to mark residual letters as expert aliases.
- [x] Refresh tests and snapshots.
- [x] Finish with verification and mark completed work.

## Verification

- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-nav-ui`

## Follow-up work

- Consider a future cross-screen selector helper if more toggle/tab panes converge on the same rendering pattern.
- Revisit whether Dashboard and Review should eventually render lightweight AI affordance cards instead of text-only cues.
