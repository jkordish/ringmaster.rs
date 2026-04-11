# Navigation Audit

This audit captures the concrete navigation problems present before the Phase 8 pass.

## Inconsistent keybindings

- `Tab` and `Shift+Tab` switch primary screens instead of moving between major regions.
- `Esc` quits the entire app instead of canceling or backing out.
- Similar controls use unrelated letters on different screens: `v`, `f`, `m`, `w`, `t`, `s`, `[`, `]`, `,`, `.`, `a`, `c`.
- Screen tabs are rendered visually but are not operated like standard keyboard tabs.

## Disjointed screen-to-screen transitions

- Top-level screen switching is globally available, but deeper movement is screen-specific and inconsistent.
- AI preflight is a special-case transient flow rather than part of a shared modal/back-out model.
- Moving from list selection to detail inspection does not follow one shared drill-down rule.

## Weak or unclear current-location cues

- The active screen is visible in the top tabs, but focused region and selected context are not consistently obvious.
- Some screens expose breadcrumb-like labels, but they do not participate in a shared orientation model.
- The footer carries many shortcuts but does little to clarify what is focused now.

## Hidden or surprising actions

- Important actions are encoded as memorized letters instead of visible, focusable controls.
- Review mode and investigation focus are visible tabs but require `v` and `f` instead of tab-like navigation.
- Trend windows and AI browser tabs are visible tabs but require non-tab semantics to operate.
- Overlay filters exist in Timeline, Explain, and Patterns, but their keyboard model is not visible from the UI itself.

## Focus traps or dead ends

- There is no explicit focus model in app state, so focus cannot be restored intentionally.
- AI preflight blocks some shortcuts, but there is no general transient focus system.
- Some screens are effectively read-only from a focus perspective, so keyboard users rely on memorized global shortcuts instead of discoverable region movement.

## Illogical focus order

- There is no shared major-region order.
- The app currently prioritizes screen switching over within-screen movement by binding `Tab` directly to screen changes.
- Controls that visually appear before lists or detail panes are not necessarily keyboard stops.

## Focus vs selection ambiguity

- Lists use a single selection marker such as `>` with no separate focused-region signal.
- Once focus conceptually leaves a list, the UI does not clearly distinguish “this item is selected” from “this region is focused”.
- Active tabs and focused tabs are not separate concepts, so manual activation is impossible.

## Actions that take too many keystrokes or too much memory

- Users must remember which screen uses `[` and `]`, which uses `j` and `k`, and which uses different letters for mode changes.
- There is no standard `Ctrl+F` or search entry point, so users must visually scan large lists every time.
- The footer tries to compensate with dense key lists, which increases memory load instead of reducing it.

## Layout problems

- Primary navigation is visible, which is good, but secondary/local navigation is often hidden in shortcut-only behavior despite available space.
- Wide layouts show tabs, panes, and lists that look interactive but do not follow standard keyboard interaction.

## Most important fixes for this pass

- Reclaim `Tab` / `Shift+Tab` for major-region traversal.
- Reclaim `Esc` for cancel/back-out instead of quit.
- Introduce one typed focus model and one typed keybinding registry.
- Turn visible tab-like controls into real keyboard composites.
- Distinguish focus from selection in rendering and state.
- Replace hard-coded footer shortcut dumps with contextual, scoped help.
