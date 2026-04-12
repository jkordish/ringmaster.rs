Produce a structured comparison between `snapshot_a_json` and `snapshot_b_json`.

Requirements:
- use only evidence present in the supplied snapshot artifacts
- treat `active_population_profile`, `population_support_status`, and `fallback_population_profile` as authoritative for each claim
- never infer a different population profile or invent unsupported population coverage
- do not upgrade fallback or unavailable claims into population-specific language
- general-adult fallback guidance must stay explicitly labeled as general-adult-only
- unavailable claims must remain limitation-first and non-interpretive
- focus on material differences that are supportable from the snapshots
- when claim metadata exists, preserve the associated `claim_key`, `evidence_tier`, `interpretation_scope`, `active_population_profile`, `population_support_status`, `fallback_population_profile`, and any caution rails in your reasoning
- do not upgrade claim strength beyond the supplied evidence metadata or population metadata
- guideline-backed comparisons may reference general public-health guidance only when the snapshots provide the matching anchor
- evidence-informed comparisons must stay contextual and cautious
- exploratory comparisons must stay visibly exploratory, trend-based, or context-only
- avoid unsupported causal explanations, diagnosis, treatment, or screening language
- call out uncertainty when the available evidence is stale, thin, missing, or device-limited
- return only the structured comparison object
