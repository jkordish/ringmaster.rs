# EVIDENCE_MODEL.md

## Purpose

`ringmaster.rs` is a local-first Oura explorer with optional bounded AI/report workflows. It is not a diagnostic product, not a treatment recommender, and not a disease-screening tool.

The evidence model defines what the product is allowed to say, how strong the scientific support is for each surfaced claim class, which populations the wording applies to, and which metrics require stronger caution rails.

The canonical implementation lives in:

- `src/evidence/registry.rs`
- `src/evidence/policy.rs`

Those files are the source of truth for CLI, TUI, snapshot export, reports, and AI outputs.

## The three-tier evidence model

Every supported metric or synthesized claim class is assigned one evidence tier.

### `guideline_backed`

Use this tier when ringmaster can anchor language to stable, high-quality public-health guidance or formal clinical guideline material that is appropriate for a general adult audience.

Current examples:

- sleep duration
- weekly physical activity totals
- weekly activity distribution framing when sufficient workout coverage exists

Allowed behavior:

- reference stable public-health guidance when the registry explicitly permits it
- describe when recent behavior appears below, within, or above that guidance range
- keep the language non-diagnostic and population-scoped

Not allowed:

- diagnosis language
- individualized treatment recommendations
- disease-screening claims
- implying that meeting guidance proves health status

### `evidence_informed`

Use this tier when the product can rely on stable scientific consensus, statements, synthesis, or bounded observational evidence, but not a strong public-health or clinical-guideline anchor for the exact product behavior.

Current examples:

- sleep timing and regularity
- resting heart rate displays and bounded trend interpretation
- HRV trend/context summaries
- VO2 max trend displays
- SpO2 trend/context handling with strong device limitations

Allowed behavior:

- cautious contextual interpretation
- within-person trend framing
- explicit uncertainty and limitation notes

Not allowed:

- hard diagnostic-style cutoffs unless the registry explicitly allows them
- clinical screening language
- overstating wearable-derived context as disease evidence

### `exploratory`

Use this tier when the surfaced signal is useful for reflection, local trend review, or pattern browsing, but scientific support is too weak or product-specific to justify stronger language.

Current examples:

- sleep score
- activity score
- readiness-like composites
- resilience-like composites
- stress-like summaries/composites
- cardiovascular-age-style metrics
- pattern-association outputs
- causal-seeming context explanations

Allowed behavior:

- explicitly exploratory wording
- trend-only or context-only framing
- clear uncertainty and non-clinical language

Not allowed:

- definitive interpretation
- causal claims
- screening, diagnosis, or treatment framing

## Population profiles and support states

Phase 9 adds typed population profiles to the registry and runtime.

Current profiles:

- `general_adult`
- `older_adult`
- `pregnancy_postpartum`
- `shift_worker`
- `athlete_high_training_load`

Every registry entry now declares support for all five profiles. Resolved descriptors expose one of three support states:

- `population_specific`
- `general_adult_only_fallback`
- `unavailable`

### `population_specific`

Use when the registry explicitly supports the active population. The product may use the claim language permitted by the tier and interpretation scope for that population.

### `general_adult_only_fallback`

Use when the registry has a general adult anchor but not a population-matched one. The product may mention the general adult anchor, but it must say that the guidance is general-adult-only and not matched to the active profile.

### `unavailable`

Use when the registry does not support interpretation for the active population. The product may still show raw metric/context values, but it must not render stronger interpretive language.

## Source hierarchy

Ringmaster prefers stable, higher-quality evidence over novelty.

The ranking used for registry authoring is:

1. formal guidelines and durable public-health guidance
2. high-quality evidence synthesis and consensus recommendations
3. scientific statements and position statements
4. device limitation material and consumer wearable caution sources
5. exploratory or product-specific observational framing

A recent isolated study does not automatically outrank stable guidance or synthesis.

## Registry fields

Every registry entry includes typed metadata for:

- `claim_key`
- source family
- evidence tier
- evidence type
- primary sources and provenance
- last reviewed date
- population support matrix for all five profiles
- fallback behavior via resolved population support state
- update cadence
- allowed wording templates
- prohibited wording categories
- whether numeric thresholds are allowed
- interpretation scope
- confidence and uncertainty requirements
- escalation/caution notes

The registry exposes compact `EvidenceDescriptor` values that are stored in snapshots and reused by reports and AI artifacts. The current serialized contract version is `ringmaster.evidence.v2`.

## Interpretation scopes

Each claim class declares how the product is allowed to interpret it.

### `cross_sectional`

The product may describe a current observed value relative to an allowed population anchor.

Current examples:

- sleep duration against general adult guidance
- weekly activity totals against general adult guidance

### `within_person_trend_only`

The product may describe how a metric is moving for the same person over time, but it must not imply a universal thresholded conclusion.

Current examples:

- resting heart rate
- HRV
- VO2 max trends
- SpO2 trends

### `contextual_only`

The product may surface the metric or context and describe local associations, but it must not offer stronger interpretation.

Current examples:

- composites such as readiness/resilience/stress
- pattern association rows
- context-derived interpretations

## Public-health anchors

Public-health anchors are only used where the registry explicitly permits them.

Current strong-anchor domains:

- sleep duration
- physical activity totals
- physical activity weekly distribution / accumulation framing

Public-health anchors are presented as guidance, not diagnosis or individualized clinical advice. When the active profile only has `general_adult_only_fallback` support, reports and AI outputs must say that explicitly.

## Sensitive domains and caution rails

Some metrics require stronger warnings because people may easily mistake them for disease-screening tools.

Sensitive metrics in the current runtime include:

- SpO2
- HRV
- readiness/resilience/stress composites
- cardiovascular-age-style metrics
- consumer sleep technology outputs

For these domains, unsupported non-general-adult population combinations resolve to `unavailable` unless the registry explicitly says otherwise.

### SpO2

SpO2 is handled as a caution-limited, trend/context metric.

Ringmaster must:

- avoid diagnostic or disease-screening phrasing
- avoid unsupported hard cutoffs unless the registry explicitly allows them
- show device limitation language
- remind the user that consumer wearable readings are useful for context/trends, not diagnosis
- avoid silently reusing stronger general-adult interpretation language for unsupported populations

### Consumer sleep technology outputs

Wearable-derived sleep outputs can be useful for local trend review, but not all sleep-stage or composite outputs support clinical-style interpretation.

Ringmaster must:

- distinguish estimated consumer outputs from diagnosis-like statements
- avoid overstating stage/composite precision
- keep language trend-based and limitation-aware where support is weak
- surface when population-specific support is unavailable

### Composite scores and resilience/readiness/stress summaries

These remain exploratory unless and until a stronger registry-backed interpretation exists.

Ringmaster must:

- label them exploratory
- avoid treatment or disease framing
- avoid implying they diagnose overtraining, illness, sleep disorders, or psychiatric conditions
- block stronger wording when the active population is unsupported

## Claims policy

The shared claims policy in `src/evidence/policy.rs` defines:

- allowed tier-specific wording
- population-aware fallback and unavailability wording
- prohibited wording categories
- uncertainty requirements
- counterevidence requirements
- mandatory disclaimers for sensitive domains
- public-health comparison helpers for strong-anchor metrics
- validation logic used to reject prohibited or population-upgraded phrasing
