# Search Modal Contract

## Goal

Make search behave like a strict modal transient so it matches help and AI preflight, without expanding search to new screens or changing search results behavior.

## Why

The last interaction inconsistency from the recent focus/transient cleanup is search still feeling lighter than the other overlays. Tightening that contract now should improve trust and keep future regressions easier to spot.

## Current state

- Search already renders as the highest-priority visible transient and restores the invoking region cleanly when it closes.
- Search does not expose an explicit transient focus contract like help and AI preflight.
- Search help text and docs still describe it more like a lightweight overlay than a strict modal.
- Regression coverage exists for search open/close and precedence, but not for transient-focus trapping or stricter modal copy.

## Desired state

- Search is treated as a strict modal transient with the same trap/restore expectations as help and AI preflight.
- The underlying focused region stays stable while search is open.
- Transient focus movements stay inside search, even though search only has one meaningful anchor in this pass.
- Search rendering and docs explicitly communicate that it is modal.

## Constraints

- Keep the current searchable scopes: Timeline events, Review cards, and AI browser items.
- Do not turn search into a richer multi-control dialog in this pass.
- Do not change dashboard layout or add test-only tracing helpers in the same change.
- Keep rendering deterministic and the repo green at the end of the pass.

## Risks

- Search input handling is split between key-event mapping and transient bindings, so small drift could break typing or shortcut priority.
- Tightening modal semantics could accidentally shadow quit or existing search result navigation if bindings are not ordered carefully.
- Snapshot tests may need small wording updates if the overlay copy changes.

## File plan

- `src/app.rs`
- `src/focus.rs`
- `src/keybindings.rs`
- `src/tui.rs`
- `docs/KEYBINDINGS.md`
- `README.md`
- `docs/execplans/20260413-search-modal-contract.md`

## Milestones

- [x] Add search-specific transient focus state and wire it through reducer behavior.
- [x] Tighten search bindings/rendering to match the strict modal contract.
- [x] Expand regression coverage for search-modal focus trapping and overlay copy.
- [x] Run formatting, linting, tests, and doctor.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

## Follow-up work

- Consider whether search should later gain a richer multi-anchor focus model with separate query/result controls.
- Keep the dashboard-only micro-spacing cleanup as a separate layout pass.
- Keep the optional test-only focus trace helper separate from behavior changes.
