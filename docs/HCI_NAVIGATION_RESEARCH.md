# HCI Navigation Research

This note summarizes the standards and guidance used for the Phase 8 navigation pass. It is intentionally scoped to high-quality primary or widely respected sources.

## Sources

- W3C WAI-ARIA Authoring Practices Guide, "Developing a Keyboard Interface"
  - https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/
- W3C WAI-ARIA APG, "Dialog (Modal) Pattern"
  - https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/
- W3C WCAG Understanding SC 2.1.2, "No Keyboard Trap"
  - https://www.w3.org/WAI/WCAG21/Understanding/no-keyboard-trap.html
- Nielsen Norman Group, "Menu Design Checklist"
  - https://media.nngroup.com/media/articles/attachments/PDF_Menu-Design-Checklist.pdf

## Keyboard operability expectations

- All interactive UI must be reachable and usable from the keyboard.
- Composite controls should generally expose one tab stop, then use arrow keys for movement within the control.
- Keyboard shortcuts should enhance navigation, not replace basic keyboard operability.
- If a control traps focus temporarily, the user must have a standard, obvious way to leave it.

## Focus order and focus visibility expectations

- Focus order should follow the logical task order, not a random visual order.
- The visual focus indicator must always be visible.
- Focus and selection are not the same thing; users need to see both states when both matter.
- When focus leaves a widget that contains a selected item, the selected state should remain visible.

## Shortcut discoverability principles

- Standard navigation must work without memorizing shortcuts.
- Shortcuts are accelerators for expert use, not the only access path.
- Help should reveal available commands in context and avoid conflicts with standard navigation.
- Showing the current scope and the keys that matter now is better than dumping a giant global hotkey list.

## Visible-navigation guidance for larger screens

- Primary navigation should stay visible on larger screens instead of being hidden behind a hamburger.
- Current location should be communicated with clear visual cues such as active highlighting, headings, and breadcrumbs or similar orientation aids.
- Local navigation for closely related content should be visible when the layout has room for it.
- Labels should be concise, descriptive, and consistent across navigation surfaces.

## Menu and choice-complexity guidance

- Large menus should stay simple, scannable, and well-grouped.
- Avoid multi-level cascading complexity where a flatter, clearer structure works.
- Navigation labels should front-load meaning and avoid internal jargon.
- Frequently used commands should be placed where they reduce physical and cognitive effort.

## Standard shortcut expectations for desktop and productivity-style UIs

- `Tab` and `Shift+Tab` are the expected way to move between major focusable regions.
- Arrow keys are the expected way to move within lists, tabsets, and similar composites.
- `Enter` and `Space` are the expected activation and commit keys.
- `Esc` is the expected close, cancel, or back-out key for transient layers.
- `Ctrl+F` is the standard discoverable entry point for find/filter behavior.
- For this TUI pass, function keys are intentionally excluded because terminal support and discoverability are less reliable than letter and control-key combinations.

## Selection vs focus and auto-activation

- WAI-ARIA APG distinguishes focus from selection and warns against hiding that difference.
- Selection can follow focus in single-select composites when updates are effectively instantaneous and low-cost.
- Selection should not automatically follow focus when moving focus would trigger expensive, disruptive, or context-changing work.
- In those cases, focus should move independently and `Enter` or `Space` should commit the new selection.

## Modal and transient behavior

- Modal dialogs should move focus inside themselves when opened.
- `Tab` and `Shift+Tab` should cycle within a modal.
- `Esc` should close the modal.
- When the modal closes, focus should return to the invoking control unless there is a stronger workflow reason to place it somewhere else.

## Working rules adopted for ringmaster.rs

- Keep visible primary navigation, especially on wide layouts.
- Make major regions tabbable in a logical task order.
- Use arrow keys within composites and reserve explicit activation for transitions that meaningfully change working context.
- Always keep current location, focus, and selection visible without relying on color alone.
- Keep shortcuts supplemental, contextual, and documented.
