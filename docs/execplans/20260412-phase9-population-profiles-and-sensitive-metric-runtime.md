# Phase 9: Population Profiles and Sensitive-Metric Runtime

## Goal

Add typed population-aware evidence resolution and stronger runtime protection for sensitive metrics across the app, reports, snapshots, and AI artifacts.

## Why

Phase 8 established the evidence registry, policy rails, and schema plumbing, but all registry-backed guidance still behaves like a single general-adult layer. Phase 9 adds explicit population scope so unsupported combinations downgrade safely instead of silently inheriting stronger language.

## Current state

- The evidence registry is typed but population handling is a single broad field rather than per-profile support.
- The runtime can attach evidence descriptors to review, report, and AI surfaces, but descriptors are not resolved against an active population profile.
- Sensitive metrics already carry caution flags, but unsupported population and metric combinations are not blocked centrally.
- Snapshots, reports, and AI artifacts carry evidence metadata, but not explicit population scope or fallback status.

## Desired state

- The app has a single active population profile configured locally and surfaced in status, doctor, and report outputs.
- Every registry-backed claim resolves through a population-aware descriptor with one of: population-specific, general-adult-only fallback, or unavailable.
- Sensitive metrics cannot silently render stronger language for unsupported population combinations.
- Snapshot and AI/report schemas encode the active population scope and resolved support status so prompts and sanitizers can enforce it.
- Maintenance checks fail cleanly when registry population coverage is incomplete or evidence entries become stale.

## Constraints

- Keep the app local-first, non-diagnostic, and compileable after each milestone.
- No automatic population inference from personal info in this phase.
- No new live sync surfaces.
- Preserve the central `Event -> Action -> State -> Render` architecture.

## Risks

- Evidence descriptor changes ripple into snapshot serialization, report rendering, and AI schema/test fixtures.
- Population-aware badges and disclaimers can become repetitive if not deduped carefully.
- Strengthening sensitive-metric guards may break existing dry-run/report fixtures until schemas and prompts are updated together.

## File plan

- `src/config.rs`
- `src/evidence/mod.rs`
- `src/evidence/registry.rs`
- `src/evidence/policy.rs`
- `src/app.rs`
- `src/lib.rs`
- `src/snapshot.rs`
- `src/report.rs`
- `src/ai.rs`
- `src/ai_prompts.rs`
- `src/ai_prompts/*.md`
- `docs/EVIDENCE_MODEL.md`
- `docs/EVIDENCE_MAINTENANCE.md`
- `docs/STATUS.md`
- `README.md`

## Milestones

- [x] Add the Phase 9 plan, guidance config, and population-aware evidence resolver.
- [x] Thread active population and resolved support status through snapshot, report, UI, and doctor surfaces.
- [x] Extend AI schemas, prompts, sanitization, fixtures, and maintenance validation for population scope.
- [x] Run full verification and update docs and status with completed work and explicit deferrals.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

## Follow-up work

- Multi-profile or inferred population handling remains deferred.
- Live SpO2 sync remains deferred.
- Population-specific guidance content beyond the explicit registry support added in this phase remains deferred.
