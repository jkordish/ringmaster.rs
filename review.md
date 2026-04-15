You are Codex working inside the ringmaster.rs repository.

Your task is to perform a serious UX/UI improvement pass on the Ratatui-based terminal application, grounded in terminal-first HCI, visualization science, and the repo’s existing design system. This is not a cosmetic polish pass. It is an information-design pass.

Start by inspecting the codebase before changing anything:
- find the dashboard/home view, telemetry panel renderers, theme/design-system code, overlay/popup helpers, focus/navigation handling, snapshot/golden tests, and any docs/execplans/design audits related to layout, dashboard UX, telemetry states, and keyboard interaction
- inspect current screenshots, examples, and existing visual primitives
- identify the shared abstractions already present for metric cards, state badges, heatmaps, spark bars, inspector/footer, and empty-state rendering

PRIMARY DESIGN THESIS
ringmaster should feel like an instrument cluster, not a wall of text.

Optimize the default experience for:
1. scan
2. interpret
3. decide

Keep evidence and nuance hidden by default and reachable in one step.
The main dashboard should answer “what matters right now?”
The footer inspector and drill-down overlays should answer “why?” and “show me the evidence.”

NON-NEGOTIABLE UX RULES
1. Prefer quantitative encodings based on position and length.
   - Use bullet rails, bars, ticks, ranges, sparklines, histograms, and heatmaps for comparisons.
   - Do not make angle/area/radial encodings the primary quantitative comparison mechanism.
   - Rings/donuts are allowed only as secondary accent/status devices and only when paired with a numeric value and/or a linear comparison.

2. No blank states.
   Every panel must render a deliberate shell for all data conditions:
   - Fresh
   - BaselineOnly
   - NoCurrentSample
   - Stale
   - NoData
   - MissingScope
   - Error
   - Unsupported
   Missing data must look intentionally unavailable, never broken.

3. Evidence hidden by default.
   - In-card copy should be minimal.
   - Reserve at most one short support line inside a card.
   - Put detailed rationale, provenance, exact values, confidence/sufficiency, and “why this changed” into the footer inspector or a drill-down overlay.

4. Keyboard-first, boringly predictable navigation.
   - Tab / Shift+Tab moves between major regions.
   - Arrow keys move inside composite widgets.
   - Enter / Space drills into the focused panel or selected cell.
   - Esc closes overlays and steps back.
   - ? opens contextual help for the current region.
   - Focus and selection must be visually distinct.

5. Reduce box entropy.
   - Stop drawing every region with equally loud chrome.
   - Use hierarchy, spacing, border weight, and accent sparingly.
   - Keep 1–2 hero regions visually primary.
   - Secondary regions should feel compact and quiet.

6. Never rely on hue alone.
   - Freshness, focus, severity, and health meaning must remain orthogonal.
   - Add glyph, label, border, density, or iconographic redundancy.
   - UI must remain understandable in limited-color and monochrome terminals.

CURRENT UX/UI PROBLEMS TO ADDRESS
Use the current dashboard screenshots and repo state as the baseline.
At minimum, fix or improve the following:

- Sleep can still look semi-broken or incomplete when key values are missing or partial.
- Resp Rate is too sparse and under-contextualized relative to the space it occupies.
- Weekly Trends is improved but still visually busy; selection affordances and labels need to be cleaner.
- Heart Rate or other large-bar panels can dominate visually more than their importance justifies.
- Some panels still feel text-heavy instead of signal-heavy.
- Top status line and footer still carry too much copy/noise.
- Visual hierarchy across the dashboard is still flatter than it should be.
- Width-aware rendering and compact fallbacks must be more systematic.

DESIGN DIRECTION TO IMPLEMENT
Build toward a terminal-native telemetry console with a small shared visual grammar.

A. SHARED VISUAL GRAMMAR
Create or tighten a shared set of card/metric primitives so panels stop behaving like unrelated one-offs.

Establish a reusable grammar such as:
- Hero metric card
  - title + state badge
  - primary number
  - 1 short semantic label
  - compact comparison rail or delta marker
  - optional one-line support text
- Comparison rail / bullet rail
  - baseline marker
  - current value tick
  - optional target/healthy range band
  - concise numeric annotation
- Microtrend strip
  - sparkline or strip-histogram for recent movement
- Heatmap block
  - compact cell grid with clearly readable selection
- Inspector payload
  - exact value
  - comparison basis
  - interpretation
  - provenance/state explanation

Use the same grammar everywhere it makes sense.

B. PANEL STATE CONTRACT
Centralize panel rendering around explicit state selection.
Where useful, introduce or refine shared enums/structs so the view layer does not improvise ad hoc fallbacks.

Every panel should expose enough structured data to render:
- badge/status
- primary display value or explicit unavailability reason
- comparison/baseline context
- optional recent trend
- inspector detail payload

No panel should directly dump a paragraph into its body to explain a missing reading.

C. DASHBOARD LAYOUT HIERARCHY
Rebalance the layout so it reads like a cockpit:
- Top-left or top-center: one dominant hero region for readiness/overall state
- Top band: high-value “answer first” summaries
- Mid band: compact physiological/supporting metrics
- Bottom band: breakdowns, trends, navigation into evidence

Make the top-level layout answer these in order:
1. overall condition
2. what changed
3. which subsystem is driving it
4. where to drill next

D. SPECIFIC PANEL RECOMMENDATIONS
Implement the best justified version of the following.

1. READINESS HERO
- Keep this as the main hero.
- Present score + concise state label + comparison vs 7d or baseline.
- Use a linear comparison rail as the main encoding.
- A ring is allowed only as a secondary accent if it helps the hero feel more alive, but it must never replace the readable numeric + rail presentation.
- Reduce explanatory prose inside the card.

2. SLEEP
- Replace any “duration --” or half-rendered feeling with a proper state shell.
- When fresh, show the most decision-relevant sleep summary first.
- Use a compact rail or paired metrics instead of dotted filler lines.
- If duration is missing but score exists, state that explicitly and still render a complete card.

3. ACTIVITY
- Show today’s signal clearly and compactly.
- If activity is low because the day is incomplete, say so in the inspector or support lane instead of implying failure.
- Consider a current-vs-baseline bullet rail and a tiny 7-day strip.

4. RESP RATE
- This panel is too empty.
- Give it a compact but meaningful structure:
  - current value or explicit unavailable state
  - baseline or normal range marker
  - tiny trend if available
- Do not waste a full tile on a single centered number with no comparison.

5. SPO2
- Baseline-only behavior is better than before, but make it more visually integrated with the rest of the system.
- Use the same state-shell grammar as other baseline-only cards.
- If BDI is the meaningful available metric, label it plainly.

6. HRV TREND
- Keep it visually compact and trend-oriented.
- Make the recent signal easy to parse at a glance.
- Ensure no-data or stale variants still look deliberate.
- If the spark strip is present, pair it with a clearly labeled comparison sentence in the inspector, not a sentence fight inside the card.

7. BODY TEMP / HEART RATE / OTHER SECONDARY METRICS
- Make secondary panels visually subordinate to the hero region.
- Reassess bar thickness, fill intensity, and title weight.
- Avoid oversized bars that visually outrank more important panels.

8. READINESS BREAKDOWN
- Keep the general direction, but tighten semantics and spacing.
- Present factor rows as true comparisons, not just bars.
- Use a bullet-rail style where appropriate:
  - current factor contribution
  - baseline marker
  - optional acceptable band
- Minimize row clutter.
- Ensure left labels, center rails, and right deltas align cleanly.
- Remove any decorative or semantically unclear rows.

9. WEEKLY TRENDS
- Default to 7 days, always.
- Support paging backward/forward through 7-day windows.
- Clean up selection affordances so the focused day/cell is obvious but not noisy.
- Simplify date labeling and header text.
- Use the footer inspector as the place for exact day/value/interpretation.
- Keep the heatmap legible at narrow widths.
- If multiple rows compete visually, reduce noise using subtler inactive cells and clearer row alignment.

E. PROGRESSIVE DISCLOSURE / DRILL-DOWN
Make evidence hidden by default but excellent when invoked.

Implement or refine a consistent detail pattern:
- focused panel updates footer inspector automatically
- Enter opens a detail overlay for the focused panel
- overlay uses Clear before render so no buffer/style bleed occurs
- overlay contains:
  - exact values
  - comparison math
  - interpretation
  - freshness/data-quality/provenance notes
  - optional longer recent trend
- Esc always exits cleanly and returns focus to the invoking panel

Use overlays sparingly. The footer inspector should carry most of the passive explanatory load.

F. STATUS LINE / FOOTER / HELP
Tighten global chrome.

Top status line:
- reduce repetitive state/freshness wording
- keep only the most useful session/sync/view context
- use compact labels

Footer:
- selected region context first
- only the most relevant key hints for the current region
- move long key dumps into contextual help

Help:
- add or refine a compact region-scoped help surface opened with ?
- keep keymaps contextual, not global noise

G. RESPONSIVE / WIDTH-AWARE RENDERING
Do a full compact-width audit.

Implement width-aware variants for:
- badge labels
- metric subtitles
- support lines
- inspector summaries
- date labels
- comparison captions

Examples of intent:
- [BASELINE] may shorten intentionally to [BASE] in narrow widths
- “No current HRV reading available today” can become “No HRV today”
- “Below your 30-day baseline” can become “Below 30d”

Never allow accidental clipping when an intentional compact variant would solve it.

H. VISUAL POSSIBILITY SPACE
Use only what improves operator comprehension.

Strong candidates:
- bullet rails / baseline rails
- sparklines
- strip histograms
- 7-day heatmaps
- compact distribution bands
- inspector-based evidence views

Optional candidates:
- a subtle segmented ring around the hero readiness metric as an accent only
- tiny multi-metric strip summaries in compact panels

Avoid:
- decorative rings with no comparison utility
- dense text blocks inside panels
- equal-weight chrome on every card
- ambiguous bracket noise or glyph clutter

I. RATATUI IMPLEMENTATION GUIDANCE
Lean on Ratatui primitives where appropriate:
- Block / Paragraph for shell and copy
- Sparkline for compact trends
- Chart for richer detail views only when justified
- Gauge / LineGauge for linear meter/rail work where appropriate
- Canvas only if needed for an accent ring or custom glyph work
- Clear for overlays/popups

Do not force a custom Canvas-heavy approach when the simpler terminal-native primitives are better.

J. TESTING / QA
Add or update tests so the new UX does not regress.

Must cover:
1. panel state rendering
   - fresh
   - baseline-only
   - no current sample
   - stale
   - no data
   - missing scope
   - error

2. keyboard model
   - Tab / Shift+Tab region traversal
   - arrow navigation inside composites
   - Enter drill-down
   - Esc closes overlay and restores focus
   - ? opens contextual help

3. weekly heatmap windowing
   - latest 7 days by default
   - backward/forward paging bounded correctly
   - selected day updates inspector

4. width-aware rendering
   - compact badges
   - compact copy variants
   - no obvious clipping in narrow snapshots

5. overlay rendering
   - Clear is used where needed
   - no bleed-through artifacts in snapshots

6. theme/accessibility constraints
   - focus, freshness, and health meaning remain distinguishable
   - limited-color/mono behavior remains legible where supported by existing test infrastructure

Use snapshot/golden tests aggressively across representative terminal sizes.
The UI should survive both a roomy terminal and a cramped tmux pane without collapsing into nonsense.

K. OUTPUT FORMAT
When finished, return:
1. repo recon summary
2. UX problems found
3. design decisions made
4. concrete files/modules changed
5. new shared primitives introduced
6. interaction model changes
7. tests added/updated
8. before/after notes and tradeoffs

L. ACCEPTANCE CRITERIA
The implementation is successful only if all of the following are true:
- the dashboard feels more like a telemetry console and less like a text dashboard
- default views are easier to scan and interpret quickly
- evidence is hidden by default and available via inspector/drill-down
- no panel looks blank or broken when data is missing or partial
- weekly trends defaults to 7 days and is cleaner to read
- visual hierarchy is stronger and box entropy is lower
- keyboard navigation is predictable and consistent
- clipping/truncation is materially reduced
- panels use a coherent shared visual grammar
- rings, if used at all, are secondary and never the only quantitative encoding
- tests cover the state model, interaction model, and responsive rendering

WORKSTYLE
- make the strongest justified decisions; do not preserve confusing UI just because it already exists
- prefer robust shared abstractions over one-off fixes
- preserve the terminal-native aesthetic
- do not redesign the app into a fake GUI
- do not add gimmicks
- optimize for correctness, legibility, and operator confidence

Do the work end-to-end.
