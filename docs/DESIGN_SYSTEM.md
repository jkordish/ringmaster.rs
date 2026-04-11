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

- `background`: terminal canvas and outer frame
- `surface_1`: primary panel background
- `surface_2`: secondary grouping surface
- `surface_3`: tertiary grouping surface or muted chrome
- `text_strong`: strongest foreground for hero numbers and active titles
- `text`: default readable foreground
- `text_muted`: metadata, annotations, inactive helpers
- `accent`: active navigation, current focal emphasis, selected analytical references
- `positive`: success and favorable change
- `warning`: stale, caution, thin-confidence, partial operator concerns
- `danger`: errors, rejected operations, hard failures
- `info`: neutral-but-important operator or context information
- `focus`: keyboard focus/selection emphasis, paired with glyph and wording

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

- `dense_gap`: one cell between tightly related rows
- `standard_gap`: default separation between sibling regions
- `section_gap`: larger break between major sections
- `panel_padding`: one-cell inset inside primary panels where the widget allows it
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

- use full bordered blocks for the strongest one or two regions on a screen
- use dividers or muted group titles for secondary regions
- avoid placing many equal-weight full boxes side by side unless the view is explicitly comparative

## Badge and chip language

Badges combine text, state prefix, and style.

- freshness badges include both wording and semantic tone
- selected/focused badges use a focus marker plus accent styling
- warnings/errors always include explicit text, not color-only signaling
- review confidence and sufficiency labels remain compact and repeatable
- AI lifecycle badges must combine text and semantics, never color alone
- preflight trust chips should surface privacy and request posture before confirmation

## Chart grammar

- use line charts for continuous temporal series
- use compact bars or value rails for ranked/discrete comparison
- use sparklines only as directional hints, never as the sole carrier of meaning
- emphasize selected points with symbol, position, and label treatment
- show missing data through gaps or explicit “no data” language
- show stale data through badge/context language adjacent to the chart, not by over-coloring the line
- keep annotation restraint: baseline and threshold markers should be fewer and clearer than raw data marks

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
