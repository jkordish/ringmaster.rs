# Phase 8: Evidence Model and Safety Rails

## Goal

Ground `ringmaster.rs` in a typed scientific evidence model that governs deterministic UI language, AI artifacts, and report outputs without shifting the product into diagnosis, treatment, or screening behavior.

## Why

The current product already has local sync, derived review signals, snapshots, optional AI analysis, reports, and evals. What it lacked was a single reusable source of truth for:

- what kinds of claims are allowed
- how strong the evidence is for each surfaced metric or interpretation
- where public-health guidance applies
- where the product must remain trend-based and cautious
- how AI and report outputs inherit the same rules as the deterministic UI

This pass closes that gap so the product becomes more trustworthy and less speculative.

## Current State At Start

- `src/review/registry.rs` defined deterministic review signals, but not a scientific evidence registry.
- `src/review/templates.rs`, `src/app.rs`, and `src/report.rs` rendered bounded language, but wording was not centrally policy-driven.
- `src/ai.rs` and `src/ai_prompts/*` prohibited diagnosis and advice at a high level, but AI artifacts did not carry explicit evidence-tier metadata.
- `src/eval.rs` checked overclaiming and medical-safety phrases, but it did not validate tier-specific scientific claims policy.
- Sleep guidance could not be grounded because the local model persisted `sleep_score`, not explicit sleep duration.
- `spo2` was tracked as a capability but remained future-ready and not locally synced.

## Shipped State

- `src/evidence/*` now provides a typed registry, evidence descriptors, and claims-policy helpers.
- Sleep duration is now ingested, stored, exported, and available to deterministic review/report logic.
- Guideline-backed interpretation is now available for sleep duration and weekly activity guidance domains.
- Sensitive or weaker domains are explicitly marked as `evidence_informed` or `exploratory` and constrained by caution rails.
- Snapshots, reports, and AI compare/review fixture artifacts now carry evidence metadata or render it directly.
- Maintenance docs and validation tests make the evidence model durable instead of ad hoc.

## Constraints

- Keep the app local-first and single-crate.
- Preserve pure Ratatui rendering and the `Event -> Action -> State -> Render` loop.
- Do not add hidden network behavior or direct widget side effects.
- No `unwrap`, `expect`, `todo!`, `panic!`, or `dbg!` in non-test code.
- Reuse the current store/snapshot/review/AI/report/eval architecture instead of building parallel flows.
- Remain explicitly non-diagnostic and non-treatment-oriented.

## Risks Managed

- Sleep guidance required a schema/store expansion for duration; this shipped with a guarded migration and snapshot compatibility defaults.
- AI artifact schema changes rippled into fixtures, evals, and report rendering; those were updated in the same pass.
- Caution language was kept compact via shared chips/badges and report rail summaries instead of large warning blocks everywhere.
- Public-health thresholds were limited to registry-backed guidance entries and labeled as general-adult guidance rather than individualized advice.

## File Plan

- `src/evidence/mod.rs`
- `src/evidence/registry.rs`
- `src/evidence/policy.rs`
- `src/lib.rs`
- `src/oura/models.rs`
- `src/oura/sync.rs`
- `src/store/migrations.rs`
- `src/store/queries.rs`
- `src/snapshot.rs`
- `src/review/registry.rs`
- `src/review/templates.rs`
- `src/review/engine.rs`
- `src/app.rs`
- `src/components/explain.rs`
- `src/components/review.rs`
- `src/components/patterns.rs`
- `src/ai.rs`
- `src/ai_prompts.rs`
- `src/ai_prompts/review_prompt_v3.md`
- `src/ai_prompts/review_task_frame_v3.md`
- `src/ai_prompts/compare_prompt_v2.md`
- `src/ai_prompts/compare_task_frame_v2.md`
- `src/ai_prompts/follow_up_prompt_v2.md`
- `src/ai_prompts/follow_up_task_frame_v2.md`
- `src/report.rs`
- `src/eval.rs`
- `tests/fixtures/ai/*`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`
- `docs/IMPLEMENT.md`
- `docs/OPENAI_INTEGRATION.md`
- `docs/EVIDENCE_MODEL.md`
- `docs/EVIDENCE_MAINTENANCE.md`

## Milestones

### Milestone 1: Evidence core and policy

- [x] Add `src/evidence/*` with typed registry entries, tier/type enums, wording rules, and validation helpers.
- [x] Add registry-backed classifications for current metric and claim families.
- [x] Add validation tests for completeness, provenance presence, and policy structure.

### Milestone 2: Sleep duration and deterministic guidance

- [x] Extend Oura sleep ingestion/store/query paths with sleep duration.
- [x] Add deterministic guideline-backed interpretation helpers for sleep duration.
- [x] Add deterministic guideline-backed weekly activity interpretation helpers.
- [x] Downgrade unsupported or weak interpretations to evidence-informed or exploratory wording.

### Milestone 3: Snapshot, TUI, and report integration

- [x] Carry evidence metadata through snapshots and surfaced view models.
- [x] Add tier badges, trend-only markers, and limitation callouts to Review / Explain / Patterns.
- [x] Update report generation to show evidence strength and limitations.

### Milestone 4: AI and eval integration

- [x] Add evidence-tier metadata to AI artifacts and prompt/schema versions.
- [x] Enforce registry/policy constraints in AI sanitation and rendering.
- [x] Expand eval graders and fixtures for evidence-tier and caution-rail compliance.

### Milestone 5: Docs and full verification

- [x] Update README and architecture/runtime docs.
- [x] Add evidence model and maintenance docs.
- [x] Run required validation, smoke commands, and fix failures before finishing.

## Verification

Full validation:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

Bounded smoke paths run during this pass:

- Guideline-backed and caution-limited report path:
  - `cargo run -- snapshot export --demo --fixture-dir tests/fixtures/phase7/strong --scope today --profile redacted --out /tmp/ringmaster-phase8-guideline-snapshot.json`
  - `cargo run -- report export --from-snapshot /tmp/ringmaster-phase8-guideline-snapshot.json --format markdown --out /tmp/ringmaster-phase8-guideline-report.md`
  - Verified the report rendered:
    - `Sleep duration -> Guideline-backed; Guidance + context; General adult sleep guidance`
    - `Sleep score [Exploratory | Trend only | Sensitive metric | Consumer wearable limitation | Not diagnostic | Not for screening | Use your baseline too]`

- Fixture-backed AI eval path:
  - `cargo run -- ai eval --fixture-dir tests/fixtures/ai --export /tmp/ringmaster-phase8-ai-eval.json`
  - Verified exported eval details carried updated schema versions and snapshot hashes.

- Fixture-backed AI metadata path:
  - `cargo run -- snapshot export --demo --fixture-dir tests/fixtures/phase7/strong --scope today --profile redacted --out /tmp/ringmaster-phase8-ai-snapshot.json`
  - `cargo run -- snapshot export --demo --fixture-dir tests/fixtures/phase7/strong --scope week --profile redacted --out /tmp/ringmaster-phase8-ai-snapshot-week.json`
  - `cargo run -- ai compare /tmp/ringmaster-phase8-ai-snapshot.json /tmp/ringmaster-phase8-ai-snapshot-week.json --fixture /home/ubuntu/ringmaster.rs/tests/fixtures/ai/compare-candidate.json`
  - Verified the rendered artifact included:
    - `claim_key: weekly_activity_minutes`
    - `evidence_tier: guideline_backed`
    - `interpretation_scope: cross_sectional`
    - caution labels / rails in both prose and JSON

## Follow-up Work

- Live `SpO₂` sync remains future-ready; this pass shipped the evidence/policy rails without adding new live sync scope.
- Population-specific guideline variants beyond general adult defaults remain deferred.
- AI fixture coverage now carries evidence metadata, but a future pass can add more fixture cases for `SpO₂`, HRV, and consumer sleep-tech warnings specifically.
- Additional guideline-backed exercise-intensity framing can expand as more bounded workout/context signals are added.
