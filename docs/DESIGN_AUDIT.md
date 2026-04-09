# DESIGN_AUDIT.md

## Purpose

This audit records the concrete visual debt present at the start of the phase-6 pass so the redesign is grounded in the current product rather than vague taste.

## Global findings

### 1. Too much identical chrome

Most regions are rendered as full bordered blocks with equal visual weight. The app reads like a wall of boxes instead of a hierarchy of primary, secondary, and tertiary information.

### 2. Weak focal hierarchy

Important information is present, but not consistently dominant. The eye has to inspect labels instead of being guided by position, grouping, and contrast.

### 3. Ad hoc style decisions

Styles are not centralized. Screen components choose borders and the few explicit colors locally, which makes it difficult to maintain a coherent visual language or test visual invariants.

### 4. Flat list-heavy presentation

Several screens communicate through generic lists even when the underlying product intent is more specific:

- Dashboard should feel editorial, but reads as a set of generic panels.
- Explain should feel evidentiary, but reads as stacked lists.
- Review should feel ranked and digest-like, but mostly looks like another list/detail panel.

### 5. Screen roles are insufficiently distinct

The app has seven screens, but many of them share the same composition grammar:

- title box
- secondary box
- list box
- notes box

That makes screen-to-screen transitions feel like tab switches inside the same layout rather than movement between intentionally different views.

### 6. States are informative but not well choreographed

Freshness, missing capability, thin data, and warning states are described accurately in text, but they are not presented with a shared visual grammar. The semantics are good; the presentation is not yet memorable.

### 7. No dedicated visual QA workflow

Deterministic rendering exists, but the product lacks:

- a user-facing snapshot command
- stable artifact output for design review
- a canonical golden snapshot review loop

## Screen-by-screen diagnosis

## Dashboard

### What is wrong

- The landing view leads with a small header instead of the actual most important story.
- Metric cards, freshness, capabilities, and “what changed” all use the same box language.
- Freshness is important but feels visually equivalent to capabilities.
- “What Changed” contains the most valuable content, but it appears late and with the same emphasis as everything else.

### Why it matters

Dashboard should pass a squint test. Right now the user has to read rather than perceive what matters first.

## Timeline

### What is wrong

- The chart exists, but the surrounding layout still behaves like stacked utility panels.
- The overlay lane, selected detail, and event list do not form a strong temporal rhythm.
- The top controls and day/filter summary take boxed space without helping the chart feel like the central instrument.

### Why it matters

Timeline should feel immersive and horizontally temporal. Right now it feels like a chart inserted into a generic admin layout.

## Trends

### What is wrong

- Trends is visually close to a stack of sparkline widgets with text below them.
- The comparison job of the screen is not obvious enough at a glance.
- Notes occupy a full boxed section rather than acting as supporting analytical annotation.

### Why it matters

Trends should feel analytical and comparison-first, not like a pile of mini dashboards.

## Explain

### What is wrong

- Explain reads as multiple equivalent lists instead of a claim-supported-by-evidence composition.
- Evidence and context are visible, but the screen does not separate argument from uncertainty sharply enough.
- Caveats are visually correct but not visually integrated into the story.

### Why it matters

Explain should feel deliberate and narrative, not like prose soup inside identical containers.

## Patterns

### What is wrong

- Patterns is a flat association list with a small filter header.
- Findings are not grouped in a way that encourages cross-day scanning.
- It is too easy to confuse Patterns with Explain because both currently depend on list-heavy boxes.

### Why it matters

Patterns should read as a browser of grouped cross-day associations, not as another evidence list.

## Review

### What is wrong

- Review correctly ranks cards, but visually it still resembles a standard list/detail screen.
- Mode and focus tabs consume vertical space without enough editorial payoff.
- Warning treatment is functionally correct but aesthetically too similar to the rest of the screen.

### Why it matters

Review should feel like the product’s briefing surface, not another dashboard variant.

## Ops

### What is wrong

- Ops has useful density, but the hierarchy between summary, diagnostics, warnings, and per-family state is weak.
- Dense information appears in long plain strings without enough structural rhythm.
- The utilitarian nature is good, but the current layout is noisier than necessary.

### Why it matters

Ops should be denser than the other screens, but it still needs strong grouping so the operator can scan without fatigue.

## Cross-screen flow issues

### 1. Header and footer continuity are underpowered

The app has a header, tabs, and footer, but they do not yet make the product feel like a single designed object. They are present, not choreographed.

### 2. Selection continuity is semantically good but visually subtle

Selected day and selected event are shared across screens, but the visual continuity of that shared context can be made much clearer.

### 3. Keyhint discoverability is accurate but crowded

The footer currently works as a dense command legend. It needs better emphasis so the user can identify what matters for the current screen without reading a compressed sentence every time.

## Design targets implied by this audit

- Reduce the number of full bordered blocks used at once.
- Make the first read on every screen obvious by position and grouping.
- Use semantic state badges and dividers consistently.
- Give each screen its own compositional role while preserving shared global chrome.
- Make deterministic visual review part of the product workflow, not just a test convenience.
