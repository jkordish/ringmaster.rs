# EVIDENCE_MAINTENANCE.md

## Goal

Keep the evidence registry current, provenance-backed, population-aware, and easy to update without turning scientific maintenance into ad hoc product copy editing.

The canonical registry is code-first and lives in `src/evidence/registry.rs`.

## Update workflow

When adding or revising a claim class:

1. Update the registry entry
- add or edit the `EvidenceRegistryEntry`
- keep the `claim_key` stable unless there is a strong migration reason
- update tier, evidence type, interpretation scope, population support matrix, wording templates, and cautions together

2. Update provenance metadata
- record primary sources
- set `last_reviewed`
- set the expected `update_cadence`
- confirm the population support matrix is still correct for all five profiles

3. Revisit policy requirements
- confirm allowed wording still matches the tier
- confirm fallback and unavailable wording are correct
- confirm prohibited wording categories are sufficient
- confirm required disclaimers/cautions are present for sensitive domains

4. Thread the change through surfaced outputs when needed
- Review
- Explain
- Patterns
- snapshot export
- AI artifact generation/sanitation
- report rendering

5. Update docs
- `docs/EVIDENCE_MODEL.md`
- `docs/EVIDENCE_MAINTENANCE.md`
- any relevant product/runtime docs if the shipped behavior changed materially

6. Run validation
- `cargo test evidence:: --lib`
- `cargo test ai:: --lib`
- `cargo test report:: --lib`
- `cargo run -- doctor`
- full suite when the change affects shipped behavior broadly

## Source expectations

Prefer stable, durable source material over novelty.

Source ranking:

1. guidelines and public-health guidance
2. evidence synthesis and consensus recommendations
3. scientific statements and position statements
4. device limitation / consumer wearable caution sources
5. exploratory framing

Do not upgrade a claim because a single recent paper looks interesting if that would conflict with stronger synthesis or public-health guidance.

## Registry hygiene checklist

Each registry entry should answer all of the following:

- What is the exact `claim_key`?
- Which evidence tier applies?
- What source family and evidence type justify that tier?
- What support status applies to each population profile?
- If fallback is allowed, what is the explicit fallback behavior?
- Is the claim cross-sectional, within-person trend only, or contextual only?
- Are numeric thresholds allowed, and if so under what policy?
- What wording templates are allowed?
- Which wording categories are prohibited?
- What uncertainty and counterevidence requirements apply?
- What caution rails or escalation notes are required?
- When was this reviewed, and how often should it be revisited?

## Validation and tests

Current guardrails include:

- registry validation tests for completeness and provenance
- full five-profile coverage checks for every registry entry
- stale-evidence checks based on `last_reviewed` and `update_cadence`
- Ops and `cargo run -- doctor` visibility for evidence-registry version and stale-review health
- policy tests for prohibited phrasing and required caution language
- runtime tests that sensitive metrics do not silently upgrade unsupported population combinations
- snapshot/report/AI tests that verify population metadata and evidence descriptors flow through the pipeline
- regression fixtures for sensitive or exploratory claim classes

If a change is hard to test, improve the design until it becomes testable.

## Versioning

The evidence registry has its own version string:

- `ringmaster.evidence.v2`

Snapshot exports persist the evidence registry version and the active population profile so downstream AI/reporting flows can tell which scientific contract produced a saved artifact.

When to bump the registry version:

- if the serialized `EvidenceDescriptor` contract changes materially
- if interpretation semantics change in a way that should be visible across saved artifacts
- if downstream consumers need an explicit signal that the scientific contract has changed

Do not bump the version for typo-level edits that do not change shipped behavior.

## Sensitive-domain checklist

Before merging changes touching a sensitive domain such as SpO2, HRV, readiness/resilience/stress composites, cardiovascular-age-style metrics, or consumer sleep tech:

- verify the claim is not diagnosis-like
- verify the wording does not imply disease screening
- verify device limitations are visible
- verify exploratory, fallback, or unavailable markers are shown where required
- verify unsupported population combinations do not silently render stronger language
- verify AI prompts and sanitizers still block clinical overreach and population hallucination
- verify reports inherit the same caution rails and population affordances

## Deferred work discipline

If a desired interpretation cannot be responsibly supported yet:

- keep or downgrade it to `exploratory`
- mark unsupported populations as `general_adult_only_fallback` or `unavailable`, whichever is appropriate
- add explicit limitation notes
- document the gap in the exec plan or status docs
- do not fill the gap with speculative copy
