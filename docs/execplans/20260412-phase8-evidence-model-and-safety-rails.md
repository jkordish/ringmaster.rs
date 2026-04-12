# Phase 8: Evidence Model and Safety Rails Audit

## Goal

Harden the shipped scientific evidence system so `ringmaster.rs` presents evidence strength, public-health anchors, population-scope limits, and caution rails more clearly across the deterministic UI, doctor/runtime diagnostics, reports, and AI-adjacent surfaces without changing the product into a diagnostic or treatment tool.

## Why This Pass Exists

The repository already ships the core Phase 8 evidence model:

- `src/evidence/*` provides a typed registry, claim policy helpers, provenance metadata, and stale-evidence checks.
- deterministic review/report UI already uses evidence tiers and caution rails
- AI prompts and artifact shaping already carry evidence and safety constraints
- docs already describe the three-tier model and maintenance workflow

What still needs tightening is the last-mile product behavior:

- sensitive review cards can hide their most important caution badges
- review detail panes do not clearly surface population fallback/unavailable status
- ops and `doctor` do not foreground evidence registry/runtime health as strongly as auth, webhook, or AI state
- regression coverage should explicitly lock these visibility and safety-rail behaviors down

This run is therefore an audit-and-hardening pass, not a greenfield implementation.

## Current State At Start

Confirmed before implementation:

- the repo is clean
- `cargo test evidence:: --lib` passes
- `cargo test ai:: --lib` passes
- `cargo test report:: --lib` passes
- `cargo run -- doctor` passes

Scientific evidence baseline already present:

- registry version: `ringmaster.evidence.v2`
- snapshot schema: `ringmaster.snapshot.v3`
- typed evidence descriptors already include tier, source family, evidence type, population support, fallback profile, caution flags, and provenance metadata
- AI/report surfaces already serialize evidence descriptors and sanitize language against the shared policy

Registry classification baseline at the start of this pass:

- `guideline_backed`
  - `sleep_duration`
  - `weekly_activity_minutes`
  - `weekly_activity_distribution`
- `evidence_informed`
  - `active_calories`
  - `steps`
  - `sleep_time_status`
  - `resting_heart_rate`
  - `hrv`
  - `vo2_max`
  - `spo2`
  - `consumer_sleep_technology`
- `exploratory`
  - `sleep_score`
  - `readiness_score`
  - `activity_score`
  - `temperature_deviation`
  - `stress_high`
  - `recovery_high`
  - `resilience_level`
  - `sleep_recovery`
  - `daytime_recovery`
  - `resilience_stress`
  - `cardiovascular_age`
  - `pattern_association`
  - `rest_mode_active`
  - `session_context`

## Constraints

- keep the app local-first and single-crate
- preserve pure Ratatui rendering and the `Event -> Action -> State -> Render` loop
- no hidden network behavior or direct widget side effects
- no `unwrap`, `expect`, `todo!`, `panic!`, or `dbg!` in non-test code
- reuse the existing store/snapshot/review/AI/report/eval architecture instead of building parallel paths
- remain explicitly non-diagnostic and non-treatment-oriented

## Risks Managed In This Pass

- avoid redesigning a working evidence model when the task is really visibility and enforcement hardening
- preserve snapshot/report/AI compatibility by extending existing runtime surfaces instead of inventing a new contract
- keep sensitive wording constraints centralized in the existing registry and policy helpers rather than duplicating rules in UI code
- keep the repo compileable after each milestone and run bounded validation before moving forward

## File Plan

- `src/app.rs`
- `src/lib.rs`
- `src/evidence/policy.rs` only if the audit proves a shared-policy gap
- `src/evidence/registry.rs` only if the audit proves a shared-registry gap
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`
- `docs/IMPLEMENT.md`
- `docs/OPENAI_INTEGRATION.md`
- `docs/EVIDENCE_MODEL.md`
- `docs/EVIDENCE_MAINTENANCE.md`
- this exec plan

## Milestones

### Milestone 1: Refresh the live plan and audit the shipped baseline

- [x] Confirm the shipped registry/policy/AI/report baseline before editing code.
- [x] Reframe this exec plan as an audit-and-hardening pass rather than a greenfield Phase 8 build.
- [x] Keep the classification baseline documented here and in product-facing docs if any tiers change during implementation.

### Milestone 2: Harden Review visibility and evidence runtime diagnostics

- [x] Prioritize sensitive caution badges for review cards so safety rails stay visible even when badge counts are capped.
- [x] Add population support and fallback visibility to review detail panes.
- [x] Surface evidence registry version and stale-evidence health in ops/runtime diagnostics.
- [x] Add the same evidence registry/runtime visibility to `cargo run -- doctor`.

### Milestone 3: Lock the behavior down with targeted regression coverage

- [x] Add tests proving sensitive review cards keep caution rails visible.
- [x] Add tests proving review detail panes show population fallback/unavailable context.
- [x] Add tests proving ops and doctor diagnostics surface evidence registry/runtime health.
- [x] Extend smoke-oriented coverage only where current AI/report fixtures miss the audited behavior.

### Milestone 4: Refresh docs to match the audited shipped state

- [x] Update docs so they describe the evidence model as shipped and explain the new visibility/runtime checks.
- [x] Keep maintenance guidance aligned with the actual registry versioning and stale-review workflow.

### Milestone 5: Full verification

- [x] Run the narrowest relevant tests after each milestone.
- [x] Run the full required validation suite before closing the pass.
- [x] Repair all failures before finishing.

## Verification

Bounded checks already confirmed at the start of this pass:

- `cargo test evidence:: --lib`
- `cargo test ai:: --lib`
- `cargo test report:: --lib`
- `cargo run -- doctor`

Required final verification:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

Required smoke paths before closing:

- one guideline-backed path that visibly renders public-health guidance wording
- one caution-limited path for `SpO₂` or another trend-only sensitive metric
- one fixture-backed AI/report path showing evidence-tier metadata flowing through prompts, artifacts, and rendering

Final verification completed on 2026-04-12:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

Smoke paths completed:

- Guideline-backed report path:
  - `cargo run -- snapshot export --demo --fixture-dir tests/fixtures/phase7/strong --scope today --profile redacted --out /tmp/ringmaster-phase8-smoke-snapshot.json`
  - `cargo run -- report export --from-snapshot /tmp/ringmaster-phase8-smoke-snapshot.json --format markdown --out /tmp/ringmaster-phase8-smoke-report.md`
  - confirmed `Weekly activity totals -> Guideline-backed`, `Sleep duration -> Guideline-backed`, and `This uses general adult public-health guidance rather than individualized clinical advice.`
- Caution-limited trend path:
  - `cargo run -- review today --demo --fixture-dir tests/fixtures/phase7/weak`
  - confirmed trend/context language plus consumer-wearable and non-diagnostic caution rails in rendered review output
- Fixture-backed AI/report metadata path:
  - `cargo run -- ai review tests/fixtures/ai/review-snapshot.json --fixture tests/fixtures/ai/review-candidate.json`
  - `cargo run -- ai runs show b2e7df3f7cf0`
  - `cargo run -- report export --from-ai-run b2e7df3f7cf0 --format markdown --out /tmp/ringmaster-phase8-ai-metadata-report.md`
  - confirmed AI output and report rendering carried `evidence: Exploratory`, safety rails, population profile metadata, and snapshot evidence registry version

Verification note:

- During final verification, `cargo run -- doctor` initially surfaced a stale dev binary after a hard-linked artifact mismatch in `target/debug/ringmaster`. A clean rebuild resolved the mismatch and the final live `doctor` output now includes `evidence_registry_version: ringmaster.evidence.v2`.

## Follow-up Work Explicitly Deferred

- expanding live `SpO₂` sync scope
- adding diagnosis, treatment, or screening behavior
- broader population-specific guideline variants beyond the current supported defaults
- large crate/workspace refactors
- visual redesign beyond the evidence visibility needed for this pass
