# Interaction Hardening + Spatial Consistency

## Goal

Harden keyboard interaction and modal behavior across the TUI, standardize cross-screen spacing rhythm, and make the Trends matrix use available width without changing the product's visual direction or screen architecture.

## Why

The current PR already established a stronger visual system, but the next visible quality gap is interaction trust: focus can feel improvised, overlays do not behave like strict modals, Trends still underuses wide layouts, and older screens still use looser shell spacing than the newer dashboard/timeline/trends surfaces.

## Current state

- `src/app.rs` keeps major-region focus and a few per-screen selections, but overlay/filter focus is still partly shared and transient behavior is split across help, search, and AI preflight.
- `src/keybindings.rs` has region-level contracts, but transient overlays still inherit some global bindings and Help is not modeled as an intentional modal focus surface.
- `src/components/trends.rs` treats sort tabs and row matrix as one focused shell, which can visually double-focus and wastes width in wide layouts.
- `src/components/explain.rs`, `src/components/patterns.rs`, and `src/components/review.rs` still render through the older `panel_block` path instead of the newer shell metrics.

## Desired state

- Each major region has a predictable tab order and each composite widget follows a consistent keyboard contract.
- Help and AI preflight behave like real modals: focus enters intentionally, background controls are not interactive, `Esc` closes them, and focus restores cleanly.
- Overlay/filter focus is screen-local instead of leaking across screens.
- Trends has an explicit internal focus model for sort tabs vs row browser and uses much more horizontal space.
- Shared shell metrics drive spacing and title rhythm across the remaining legacy screens.

## Constraints

- Keep the current screen list, dashboard structure, panel layout model, and semantic pastel visual direction.
- Do not add network or store behavior to UI components.
- Keep deterministic rendering and snapshot coverage intact.
- Do not leave the tree failing clippy/tests/doctor at the end of the pass.

## Risks

- Interaction changes touch state, bindings, rendering, and tests at once, so drift between them could cause regressions.
- Replacing shared spacing helpers can perturb snapshots on multiple screens, including ones not directly targeted by the pass.
- Modal tightening can accidentally hide useful emergency shortcuts if transient scopes are reduced too aggressively.

## File plan

- `src/action.rs`
- `src/app.rs`
- `src/focus.rs`
- `src/keybindings.rs`
- `src/navigation.rs`
- `src/tui.rs`
- `src/ui/chrome.rs`
- `src/ui/layout.rs`
- `src/components/ai.rs`
- `src/components/explain.rs`
- `src/components/patterns.rs`
- `src/components/review.rs`
- `src/components/trends.rs`
- `docs/KEYBINDINGS.md`
- `docs/execplans/20260413-interaction-hardening-spatial-consistency.md`

## Milestones

- [x] Add the exec plan plus shared interaction primitives for transient/modal focus and composite child focus.
- [x] Refactor app state and keybindings to use screen-local composite memory and strict transient scope handling.
- [x] Update Trends, Help, and AI preflight to use the new focus/transient model and correct modal rendering.
- [x] Bring Explain, Patterns, and Review onto the shared shell spacing rhythm and tighten remaining dashboard/trends layout issues.
- [x] Expand keyboard/snapshot regression coverage and run full verification.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --screen dashboard --screen trends --screen explain --screen patterns --screen review --screen ai --size medium --size wide --out-dir /tmp/ringmaster-interaction-pass`

Status:
- Passed on 2026-04-13 after the shared focus/transient pass landed.

## Follow-up work

- Consider whether search should graduate from a lightweight transient to a richer modal/browser surface after this pass stabilizes.
- Revisit whether Timeline lanes would benefit from a distinct child-focus model if overlay families gain more depth in a later release.
