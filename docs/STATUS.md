# STATUS.md

## Purpose

This file is the current truth for the repository during the phase-3 context-overlays-and-explainability pass. It records what now works, what drift was removed, and what remains intentionally deferred.

## Baseline audit at start of this pass

Verified on `2026-04-08` before implementation:

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
- local-first poll architecture
- background refresh already reused the same scheduler core as `sync watch`

Repository gaps at baseline:

- context-family placeholders existed, but the product still centered on metrics alone
- Timeline overlays were placeholder text rather than a real read model
- there was no selected-day story screen
- no deterministic pattern surfacing for workouts, tags, or sessions
- no rebuild command for derived overlays and analytics

## Current implemented truth

The repository now includes:

- real sync, persistence, and fixture coverage for `workouts`, `enhanced_tags`, and `sessions`
- family-aware scheduler coverage and freshness semantics for all six supported families
- persisted `derived_context_events` and `derived_pattern_summaries` tables
- automatic bounded recent-window derived-table refresh after successful non-dry-run syncs
- a deterministic `derive rebuild` command with `--demo` and `--fixture-dir` support
- a shared selected-day concept across Dashboard, Timeline, and Explain
- a shared selected-event concept across Timeline and Explain
- a Timeline screen with gap-aware heartrate rendering, real overlay lanes, family toggles, selected-event details, and a selected-day event list
- an Explain screen with selected-day summary lines, measurement context, evidence bullets, context entries, and explicit caveats
- a Patterns screen with descriptive association rows, sample counts, magnitude, and sufficiency buckets
- clear missing-scope and insufficient-history messaging instead of silent emptiness
- deterministic wording rules that avoid causal claims and medical framing

## Supported data families

Live sync and persistence currently cover:

- `personal`
- `daily`
- `heartrate`
- `workout`
- `enhanced_tag`
- `session`

Legacy `tags` remain part of the canonical context-event read model when present in SQLite, but the product is intentionally centered on `enhanced_tag`.

## Explainability and analytics truth

Explain and Patterns are intentionally restrained:

- Explain summarizes what was measured, what context events occurred, and what evidence exists around the selected day
- Patterns surface descriptive associations only after a minimum sample threshold
- the UI always shows `n`
- thin history is called out explicitly
- wording uses “associated with”, “co-occurred with”, and “after days with”
- the product intentionally avoids causal claims, medical advice, and significance theater

## Milestone tracker

- [x] Milestone 1: add the phase-3 plan, schema/config/scheduler foundations, and real sync/store coverage for `workouts`, `enhanced_tags`, and `sessions`
- [x] Milestone 2: add persisted derivation for canonical context events and pattern summaries plus the `derive rebuild` CLI path
- [x] Milestone 3: unify selected-day and selected-event state across the TUI, upgrade Timeline with overlays and drill-down, and add Explain + Patterns screens
- [x] Milestone 4: add meaningful migration/sync/derivation/analytics/UI tests and align docs with the implemented behavior
- [x] Milestone 5: run the final verification sweep and repair any failures before closeout

## Verification completed in this pass

Verified on `2026-04-08` after implementation:

- `cargo fmt --all --check` passed
- `cargo clippy --all-targets --all-features -- -D warnings` passed
- `cargo test --all` passed
- `cargo run -- doctor` passed
- `cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3` passed
- `cargo run -- derive rebuild --demo` passed
- `cargo run -- demo` passed
- `cargo run -- sync watch --demo --max-iterations 1` passed

## Tests now in place

The phase-3 pass now includes meaningful coverage for:

- migration application and record-count expectations
- phase-2-to-phase-3 migration backfill for existing workout/session rows
- fixture-backed sync for workouts, enhanced tags, and sessions
- canonical context-event derivation and persisted analytics rebuilds
- selected-day and selected-event app-state behavior
- timeline family filters and event selection
- Explain rendering
- Patterns insufficient-data rendering
- non-interactive TUI smoke rendering

## Known intentional deferrals

- webhook receiver and webhook subscription lifecycle
- broader Oura data surface outside the current personal/daily/heartrate/context-family slice
- exports, sharing, and reporting flows
- packaging, installers, and release automation
- generalized machine learning or medical interpretation
