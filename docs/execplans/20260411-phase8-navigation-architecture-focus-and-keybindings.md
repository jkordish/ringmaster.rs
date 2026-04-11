# Phase 8 Navigation Architecture, Focus, and Keybindings

## Goal

Standardize navigation in `ringmaster.rs` around a visible, keyboard-first, predictable interaction model with explicit focus, scoped keybindings, coherent drill-down/back-out rules, and regression coverage.

## Why

The app has real product depth now, but navigation is still encoded as screen-specific shortcuts and ad hoc conventions. That increases learning cost, hides capability, and makes similar screens behave differently. This pass reduces interaction friction without expanding product scope.

## Current state

- Top-level screen switching is centralized in `src/tui.rs`, but it is driven by a large ad hoc key switch.
- `Tab` currently switches screens instead of moving between regions.
- `Esc` currently quits the app instead of canceling or backing out.
- Many important controls are visible but not keyboard components: trend windows, review mode/focus tabs, AI browser tabs.
- Focus is not modeled in `AppState`; selection is mostly screen-local state.
- Focus and selection are not consistently distinguishable once focus leaves a list or tabset.
- Footer help is hard-coded, screen-specific, and biased toward memorized shortcuts.
- Search/find is not standardized.

## Desired state

- Primary navigation stays visible, especially on wide layouts.
- `Tab` / `Shift+Tab` moves between major regions; arrows move within composites.
- `Enter` / `Space` activates; `Esc` closes or backs out one layer; `Ctrl+F` opens search in the current context.
- Function keys are not required.
- One keybinding registry defines global, screen, region, and transient bindings plus expert aliases.
- Focus is explicit in state, region order is logical, and focus restoration works after help/search/modal dismissal.
- Pane types are explicit in behavior: selectors use selector keys, lists use list keys, chart/pager regions use timeline/day keys, and detail panes use explicit return semantics instead of screen-specific shortcuts.
- Search, help, and detail drill-down follow one grammar across Dashboard, Timeline, Trends, Explain, Patterns, Review, AI, and Ops.

## Constraints

- Local-first only.
- Single-package crate.
- UI remains pure; no DB/network side effects in components.
- Preserve the existing `Event -> Action -> State -> Render` flow.
- No blocking work on the render path.
- No `unwrap`, `expect`, `dbg!`, `todo!`, or `panic!` in non-test code.
- No color-only semantics.
- No keybinding sprawl or modal labyrinth.
- No function keys for the standard model.

## Risks

- The current reducer and render models are screen-centric, so adding focus/search state could sprawl unless it stays typed and centralized.
- Some screens are mostly read-only today, so region design must avoid fake interactivity.
- Search must feel useful where it exists without forcing every surface to support it.
- Replacing the current footer and key switch could break snapshot tests unless the new help system is deterministic.

## File plan

- `src/action.rs`
- `src/app.rs`
- `src/tui.rs`
- `src/navigation.rs` (new)
- `src/keybindings.rs` (new)
- `src/ui/chrome.rs`
- `src/components/{dashboard,timeline,trends,explain,patterns,review,ai,ops}.rs`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`
- `docs/IMPLEMENT.md`
- `docs/HCI_NAVIGATION_RESEARCH.md`
- `docs/NAVIGATION_AUDIT.md`
- `docs/KEYBINDINGS.md`

## Milestones

- [x] Write the research note, navigation audit, and initial keybinding reference.
- [x] Add typed navigation state and a centralized keybinding registry.
- [x] Refactor event mapping and reducer behavior to the canonical grammar.
- [x] Update chrome/components for visible focus, selection, help, and search states.
- [x] Add regression tests, snapshot coverage, and deterministic smoke flows.
- [x] Update project docs and run full validation, with the remaining repo-wide clippy backlog recorded explicitly.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-nav-ui`

## Validation notes

- `cargo fmt --all --check` passed.
- `cargo test --all` passed after the navigation and entrypoint cleanup work, including the deterministic navigation smoke path and the existing CLI smoke suite.
- `cargo run -- doctor` passed.
- `cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-nav-ui` passed.
- `cargo clippy --all-targets --all-features -- -D warnings` still fails on a broad repo-wide pedantic/nursery/cargo backlog outside the navigation surface. The pass fixed the navigation-specific clippy findings it introduced and cleaned up several nearby entrypoints and helpers, but the remaining blockers now mostly fall into two buckets:
  - transitive `clippy::cargo` `multiple_crate_versions` findings caused by dependency ecosystem splits such as `reqwest`, `thiserror`, `sha2`, `windows-*`, and related crates
  - older repo-wide lint debt such as `future_not_send`, `missing_errors_doc`, `too_many_lines`, and numeric cast/style issues across modules like `src/ai.rs`, `src/app.rs`, `src/lib.rs`, `src/oura/*`, `src/store/queries.rs`, and webhook support code

## Follow-up work

- Optional user-configurable keybinding overrides.
- More advanced search/filtering on currently read-mostly screens if real usage warrants it.
- Any post-pass visual refinements that are purely presentational and not part of the core navigation standardization.
