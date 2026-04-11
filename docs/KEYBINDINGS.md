# Keybindings

This document defines the canonical keyboard model for `ringmaster.rs`.

## Standard navigation

- `Tab`: move to the next major region
- `Shift+Tab`: move to the previous major region
- `Left` / `Right`: move within tabsets, pagers, and horizontal composites
- `Up` / `Down`: move within lists and vertical composites
- `Home` / `End`: jump to the first or last item in the focused composite
- `PageUp` / `PageDown`: make a larger contextual jump where supported
- `Enter` / `Space`: activate or commit the focused control
- `Esc`: close help, close search, dismiss a transient, or back out one interaction layer
- `Ctrl+F`: open find in the current searchable context
- `?`: open contextual keyboard help
- `Ctrl+C`: quit the application

## Search behavior

- `Ctrl+F`: open search for the current searchable region or its default screen list
- While search is active:
  - `Enter`: move to the next result
  - `Shift+Enter`: move to the previous result
  - `Backspace`: delete one character
  - `Esc`: close search and restore prior focus

## Top-level navigation

- Primary screen navigation remains visible as tabs.
- Focus the tab row with `Tab`.
- Use `Left` / `Right` / `Home` / `End` to move across top-level screens.
- Use `Enter` or `Space` to activate the focused screen.

## Pane semantics

The app now treats pane movement by pane type instead of by screen-specific shortcut history.

- `Views` and selector panes:
  - `Left` / `Right` move between options
  - `Home` / `End` jump to the first or last option
  - `PageUp` / `PageDown` jump to the selector edges when a larger jump is meaningful
  - `Enter` / `Space` commit when the selector has an explicit activation step
- List panes:
  - `Up` / `Down` move one item
  - `Home` / `End` jump to the first or last item
  - `PageUp` / `PageDown` move by a larger chunk
  - `Enter` / `Space` inspect, launch, or drill into the selected item
- Chart and pager panes:
  - `Left` / `Right` move within the current series or window
  - `Home` / `End` jump to the first or last point
  - `PageUp` / `PageDown` move to the previous or next larger time window such as the selected day
- Detail panes:
  - `Enter` / `Space` return to the invoking list or detail source when the pane is a read-only drill-down
  - `Esc` always backs out one region layer at a time

Back-out is now region-ordered instead of screen-specific: `Esc` walks to the previous major region on the current screen before it ever returns to `Views`.

## Expert aliases

Expert aliases are optional accelerators. They do not replace the standard model and are not required for operation.

- `h` / `l`: horizontal movement within tab-like composites
- `j` / `k`: vertical movement within list-like composites
- `/`: open search
- `g` / `G`: first or last item in a list-like composite
- `1` through `8`: direct jump to top-level screens

## Notes

- Function keys are intentionally not part of the standard model for this pass.
- Old one-off navigation shortcuts such as bracket-based paging and screen-specific mode letters are intentionally not part of the canonical navigation model anymore.
- Screen-specific behavior is documented through the in-app help overlay and footer generated from the centralized registry.
