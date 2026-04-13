# DESIGN_SYSTEM.md

## Purpose

This document defines the visual system used by the current terminal redesign. It is the source of truth for how the app communicates hierarchy, state, rhythm, trust, and guided AI behavior.

## Design goals

- at-a-glance comprehension first
- restrained, memorable visual identity
- hierarchy before decoration
- useful density without crowding
- coherent cross-screen continuity
- deterministic snapshotability

## Palette roles

The palette is semantic rather than screen-specific. Code should reference roles, not literal colors.

Surfaces:

- `surface_base`: terminal canvas and outer frame
- `surface_panel`: default panel background
- `surface_panel_alt`: secondary grouping surface or muted chrome

Lines:

- `line_subtle`: internal separators and quiet shell edges
- `line_normal`: standard panel shell
- `line_strong`: outer frame and explicit emphasis

Text:

- `text_primary`: strongest foreground for hero values and selected focal text
- `text_secondary`: default readable foreground
- `text_tertiary`: metadata, annotations, inactive helpers
- `text_disabled`: unavailable or suppressed text

States:

- `focus_accent`: keyboard focus/selection emphasis
- `state_fresh`: fresh or healthy data state
- `state_warn`: stale, caution, or partial confidence
- `state_error`: hard failures
- `state_info`: neutral-but-important operator context
- `accent`: active navigation and selected analytical references

Critical rule:

- focus and freshness must remain visually distinct; a focused stale panel and a fresh unfocused panel should not collapse into the same accent family

AI-specific interpretation:

- `accent` and `focus` call out the active AI workbench slice or selected saved artifact
- `warning` communicates privacy or provider concerns before launch
- `info` communicates trust metadata such as stateless mode, disabled tools, and payload scope

## Text hierarchy

- `hero`: large numerical or status focal point
- `section_title`: major region titles
- `label`: metric names, tab labels, category names
- `body`: default content text
- `annotation`: supporting explanations and callouts
- `metadata`: timestamps, hints, provenance, secondary diagnostics

## Spacing rhythm

Use semantic spacing rules rather than per-screen magic numbers:

- `micro_gap`: one cell between tightly related rows
- `panel_padding`: two-cell default inset where the viewport allows it
- `section_gap`: larger four-cell break reserved for major internal compositions
- `compact_density`: prefer stacked sections and fewer side rails
- `wide_density`: prefer lateral comparisons and inspector sidecars

## Border and divider language

Not every grouping needs a full border.

- `hero panel`: full bordered block for the primary focal region
- `section panel`: full or partially emphasized block for a major section
- `subtle panel`: bordered grouping used when a full panel still improves scanability
- `divider group`: separated by spacing and title treatment instead of another border
- `list section`: uses row rhythm and selection affordances first, borders second

Rules:

- outer app frame carries the strongest line weight
- primary panel shells use normal line weight
- internal separators should prefer subtle lines or whitespace over nested boxes
- focused panels use stronger line weight and `focus_accent`
- fresh/stale/error state is communicated by badge and tone, not by stealing focus styling
- avoid placing many equal-weight full boxes side by side unless the view is explicitly comparative

## Badge and chip language

Badges combine text, state prefix, and style.

- freshness badges include both wording and semantic tone
- selected/focused badges use a focus marker plus accent styling
- warnings/errors always include explicit text, not color-only signaling
- review confidence and sufficiency labels remain compact and repeatable
- AI lifecycle badges must combine text and semantics, never color alone
- preflight trust chips should surface privacy and request posture before confirmation
- title bars should share one optical system: consistent inset, badge alignment, and title-to-body spacing

## Chart grammar

- use line charts for continuous temporal series
- use compact bars, value rails, or stacked profiles for ranked/discrete comparison
- use sparklines only as directional hints, never as the sole carrier of meaning
- emphasize selected points with symbol, position, and label treatment
- show missing data through gaps or explicit “no data” language
- show stale data through badge/context language adjacent to the chart, not by over-coloring the line
- keep annotation restraint: baseline and threshold markers should be fewer and clearer than raw data marks
- each panel should prefer one rendering vocabulary at a time: bars, rails, blocks, or braille-like density, rather than mixing all of them

## State language

Every state should be legible in monochrome and in low-color terminals.

- `fresh`: affirmative label + positive tone
- `stale`: explicit stale wording + warning tone + contextual detail
- `syncing`: active wording + focus/info tone
- `empty`: neutral empty block with next-step hint
- `missing capability`: explicit capability wording + muted or warning framing
- `insufficient history`: explicit “thin” or “not enough data yet” wording
- `error`: concise hard-failure block with danger tone
- `selected`: focus marker, ordering emphasis, and accent/focus tone
- `disabled`: muted text plus explicit unavailable wording
- `queued`: explicit queued wording plus info/focus tone
- `running`: active wording plus focus tone
- `succeeded`: explicit success wording plus positive tone
- `failed`: explicit failure wording plus danger tone
- `cancelled`: explicit cancelled wording plus warning tone
- `interrupted`: explicit interrupted wording plus warning tone and recovery context

## Screen roles

- `Dashboard`: editorial front page with “what matters now” as the dominant focal region
- `Timeline`: immersive temporal instrument with chart-first composition
- `Trends`: comparison matrix and drift scanner
- `Explain`: narrow evidence view with claim, support, and uncertainty
- `Patterns`: grouped cross-day association browser
- `Review`: ranked editorial digest and bounded investigation surface
- `AI`: workbench for launch, preflight, saved-run inspection, artifact browsing, and guided follow-up actions
- `Status`: utilitarian diagnostic console with disciplined hierarchy

AI workbench composition rules:

- the top region should explain the product boundary, not mimic chat chrome
- launch points belong in a compact, clearly guided list
- browser tabs should keep snapshots, runs, and reports visually related
- detail panes should foreground provenance and actionability over raw JSON
- preflight overlays must read as trust and confirmation surfaces, not as generic modal clutter

## Trust surfaces

AI-related trust state must be obvious without opening external docs.

Required cues:

- snapshot-first wording near the AI workbench header
- explicit stateless and tools-disabled wording in preflight and detail views
- visible privacy profile on preflight, saved runs, and report details
- artifact path or source linkage when local files are involved
- warnings rendered with words and layout, not just hue

The visual system should make AI feel inspectable and bounded rather than magical.

## Terminal size adaptation

Three breakpoint classes are canonical:

- `compact`: approximately `90x28`
- `medium`: approximately `120x36`
- `wide`: approximately `160x44`

Rules:

- compact prioritizes vertical flow and removes non-essential side-by-side comparisons
- medium keeps the main structure but limits tertiary columns
- wide uses sidecars, comparison rails, and larger temporal layouts without adding decorative filler

AI-specific adaptation:

- compact keeps launch points, browser list, trust panel, and detail in a vertical-first composition
- wide adds a clearer left rail for launch and browsing, plus a right-side trust and detail inspector
- preflight overlays should stay legible at compact widths and avoid hiding the fact that confirmation is required

## Snapshot QA

`ringmaster ui snapshot` is the canonical visual QA path.

It should:

- render deterministic screen snapshots from demo or fixture-backed data
- support multiple screens and multiple terminal sizes
- write stable text artifacts to an output directory
- be usable for human review and regression tests

Phase-9 AI QA extends this to cover:

- provider-disabled AI workbench state
- preflight confirmation state
- running and saved-run inspection states
- browser detail for snapshots, runs, and reports
- failure and cancellation rendering

## Intentionally deferred

- image-based screenshot export
- rich animation systems
- alternate theme packs
- decorative icon packs
- any redesign that widens the product’s data or workflow scope
