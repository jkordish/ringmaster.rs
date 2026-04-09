# STATUS.md

## Purpose

This file is the current truth for the repository during the phase-5 smart-reviews-and-guided-investigations pass. It records what now works, what gaps this pass closed, and what remains intentionally deferred.

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
- bounded derived context overlays, explainability, and pattern summaries

Repository gaps at baseline:

- no ranked daily or weekly review workflow
- no bounded guided investigation flow
- no canonical registry for reviewable signals
- no persisted review feature snapshots
- no TUI smart surface above Explain and Patterns
- no CLI family for deterministic review output
- limited use of Oura families that materially strengthen daily and weekly review quality

## Current implemented truth

The repository now includes:

- normalized sync, store, fixture, and demo coverage for:
  - `daily_stress`
  - `daily_resilience`
  - `sleep_time`
  - `daily_cardiovascular_age`
  - `vo2_max`
  - `rest_mode_period`
- a canonical review signal registry that defines:
  - source family
  - granularity
  - baseline window
  - directionality
  - evidence kind
  - required capability
  - safe wording constraints
  - allowed smart surfaces
- persisted `derived_review_signal_days` rows rebuilt from local stored data
- bounded recent-window review snapshot refresh during derivation and full rebuild support through `derive rebuild`
- a deterministic review engine that produces:
  - ranked Today review cards
  - ranked Week review cards
  - explicit evidence and counterevidence
  - confidence and sufficiency labels
  - “why this is shown” explanations
- a bounded investigation engine for these focuses:
  - readiness
  - sleep
  - recovery
  - stress
  - activity
- deterministic templates shared by CLI and TUI
- a new `Review` TUI screen with:
  - Today mode
  - Week mode
  - Investigate mode
  - ranked card selection
  - evidence detail
  - warning detail
- a canonical `review` CLI family:
  - `review today`
  - `review week`
  - `review investigate`
- concise smart-summary reuse in existing surfaces:
  - Dashboard top insight reuse
  - weekly drift note in Trends
  - Review hint in Explain

## What “smart” means here

This pass deliberately does not add a chat assistant.

The implemented smart layer is:

- deterministic
- local-data-backed
- capability-aware
- template-based
- explicit about weak evidence

The implemented smart layer is not:

- freeform chat
- hidden heuristic prose generation
- causal inference
- medical interpretation
- hosted AI

## Review and investigation truth

Today review:

- anchors on the selected day
- compares direct evidence signals against prior comparable history
- ranks observations using explicit scoring factors

Week review:

- anchors on the selected day and reviews the trailing 7-day window
- compares that window against a prior 28-day baseline window
- highlights positive changes, negative drifts, and unresolved anomalies

Investigation:

- stays bounded to fixed focuses
- reuses ranked review cards rather than inventing a new reasoning stack
- surfaces evidence bundles, counterevidence bundles, warnings, and “look next” pointers

Confidence and sufficiency:

- remain separate concepts
- sufficiency reflects comparable-history volume
- confidence reflects sufficiency plus freshness plus evidence balance
- thin or stale data never renders as high confidence

## Tests now in place

The phase-5 pass now includes meaningful coverage for:

- migration application for review-support tables and review snapshot tables
- fixture-backed sync for the six review-support Oura families
- review registry behavior
- review feature snapshot shaping
- today review ranking
- bounded investigation assembly
- deterministic template wording
- Review screen rendering and key mapping
- CLI parsing and demo-output smoke coverage for the review family

## Verification completed in this pass

Verified on `2026-04-09` after implementation:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- review today --demo`
- `cargo run -- review week --demo`
- `cargo run -- review investigate --focus readiness --demo`
- `cargo run -- derive rebuild --demo`

## Known intentional deferrals

- freeform chat or open-ended assistant prompts
- runtime LLM dependencies or hosted AI services
- recommendation or coaching systems
- notifications
- packaging, installers, and release automation
- broad theming and UI redesign
- broader Oura family expansion beyond the six review-support additions in this pass
