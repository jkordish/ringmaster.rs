# STATUS.md

## Purpose

This file is the current truth for the repository during the phase-7 scenario-hardening-and-state-coverage pass. It records what now works, what gaps this pass closed, and what remains intentionally deferred.

## Baseline audit at start of this pass

Verified on `2026-04-09` before implementation:

- `cargo fmt --all --check` passed
- `cargo clippy --all-targets --all-features -- -D warnings` passed
- `cargo test --all` passed
- `cargo run -- doctor` passed
- `cargo run -- sync watch --demo --max-iterations 1` passed

Repository strengths at baseline:

- real local OAuth login and real one-shot sync
- deterministic demo mode and useful snapshot rendering
- SQLite-backed typed store/query seams
- honest live empty/error states
- local-first poll plus webhook freshness architecture
- bounded derived context overlays, explainability, pattern summaries, and smart reviews

Repository gaps at baseline:

- demo snapshots were strong enough for smoke coverage, but fixture-backed scenario coverage was still shallow
- `ui snapshot` could render only one fixture-backed app state at a time
- screen regression coverage did not exercise canonical strong, weak, empty, stale, and error states across the full screen set
- selected-day continuity still jumped to the newest day when the current day disappeared after reload
- uncertainty and missing-capability copy still had a few ambiguous or awkward phrases

## Current implemented truth

The repository now includes:

- a new design audit in `docs/DESIGN_AUDIT.md` with explicit screen-by-screen hierarchy and clutter findings
- a centralized design-system reference in `docs/DESIGN_SYSTEM.md`
- an internal presentation layer under `src/ui/*` for:
  - semantic palette roles
  - state tones
  - text emphasis helpers
  - breakpoint-aware layout helpers
  - shared panel, badge, and title-row chrome
  - shared chart styling helpers
  - deterministic snapshot artifact generation
- a dedicated `ui snapshot` CLI family for deterministic design QA
- a canonical phase-7 fixture root under `tests/fixtures/phase7` with `strong`, `weak`, and `empty` seed states plus code-driven `stale` and `error` overlays
- deterministic snapshot output across these viewport classes:
  - `compact` = `90x28`
  - `medium` = `120x36`
  - `wide` = `160x44`
- deterministic scenario-matrix output across these scenario classes:
  - `strong`
  - `weak`
  - `empty`
  - `stale`
  - `error`
- redesigned screen choreography for:
  - Dashboard as the editorial front page
  - Timeline as the immersive temporal view
  - Trends as the comparative scanning matrix
  - Explain as the deliberate evidence view
  - Patterns as the grouped association browser
  - Review as the editorial digest
  - Ops as the utilitarian operator console
- shared state styling for:
  - selected
  - focused
  - stale
  - syncing
  - empty
  - missing capability
  - warning
  - error
  - de-emphasized metadata
- non-color-only state emphasis through wording, badges, prefixes, ordering, and chrome
- tighter copy for:
  - thin-history uncertainty
  - missing capability states
  - empty local-cache states
  - stale receiver/subscription/sync states
- selected-day restoration that prefers nearest continuity instead of defaulting straight to newest data
- lightweight breadcrumbs on Timeline, Explain, and Review when they reduce cognitive load around shared day and linked-event context
- snapshot and smoke coverage for:
  - core screen rendering
  - compact / medium / wide sizing
  - canonical strong / weak / empty / stale / error scenario shaping
  - `ui snapshot` artifact plumbing
  - selected-card and screen-role regressions
  - scenario-tagged smoke output

## What changed in this pass

Dashboard:

- now has one clear focal point: “what matters now”
- treats metric cards and freshness/capability context as secondary rhythm
- uses drill-down cues as tertiary navigation instead of another block wall
- now has canonical strong, weak, empty, stale, and error fixture coverage in the snapshot matrix

Timeline:

- leads with the chart rather than splitting attention equally with side panels
- uses overlay lanes and selected detail to reinforce temporal reading
- keeps compact terminals usable instead of collapsing into crushed sections
- shows a lightweight breadcrumb when the selected day or linked event context would otherwise be easy to lose

Trends:

- now reads like a comparison surface instead of a stacked card list
- uses baseline relationships, deltas, and compact spark hints for scanability

Explain:

- now emphasizes the claim first, then measured inputs, then evidence and uncertainty
- feels deliberately narrower and more narrative without turning into prose soup
- labels prior-day carryover explicitly so late events are not mistaken for same-day evidence

Patterns:

- now groups findings and interpretation so it reads differently from Explain at a glance
- uses cleaner relation copy instead of awkward “associated with lower ...” phrasing

Review:

- now feels like an editorial digest with ranked observations and bounded brief detail
- keeps selected day, review mode, and focus visible through a lightweight breadcrumb

Ops:

- remains dense, but diagnostics no longer compete equally with every other element
- uses sharper grouping for summary, family status, diagnostics, and warnings
- now has explicit strong, weak, empty, stale, and error regression coverage through fixture-backed snapshots

## Design-system truth

The code now standardizes:

- palette roles:
  - background
  - layered surfaces
  - strong/default/muted foreground tiers
  - accent
  - positive
  - warning
  - danger
  - info
  - focus
- typography/emphasis roles:
  - hero
  - section title
  - label
  - body
  - annotation
  - metadata
- spacing rhythm:
  - compact internal gaps
  - standard block spacing
  - section separation
  - viewport-specific density rules
- chrome language:
  - full panels for primary regions
  - dividers and implied grouping for secondary regions
  - consistent badge and chip treatment
  - standardized focus and state emphasis
- chart grammar:
  - lines for time
  - compact bars for discrete comparison
  - sparklines for directional hints
  - consistent missing-data and stale-data treatment

## Tests now in place

The phase-7 pass now includes meaningful coverage for:

- semantic theme and layout helpers
- deterministic snapshot size selection
- `ui snapshot` artifact generation
- CLI parsing for the new `ui snapshot` surface
- compact vs wide screen rendering expectations
- selected-state emphasis that does not depend on color alone
- selected-day continuity after live snapshot replacement
- carryover labeling and missing-capability copy regression tests
- phase-7 fixture-root smoke coverage with scenario-tagged artifact assertions
- smoke coverage for design-QA snapshot output

## Verification completed in this pass

Verified on `2026-04-09` after implementation:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-ui-snapshots`
- `cargo run -- ui snapshot --fixture-dir tests/fixtures/phase7 --screen dashboard --screen explain --screen review --screen ops --size compact --size wide --out-dir /tmp/ringmaster-ui-snapshots-phase7-smoke`

## Known intentional deferrals

- new Oura analytics families beyond the existing live and review-support surface
- freeform chat or hosted AI assistance
- PNG/image snapshot export; text snapshots remain the canonical visual QA surface
- richer animation or transition effects
- notifications
- packaging, installers, and release automation
