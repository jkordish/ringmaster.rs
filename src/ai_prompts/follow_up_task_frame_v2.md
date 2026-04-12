Produce a structured follow-up for the provided `source_artifact_json` using the supplied snapshot artifact JSON.

Requirements:
- use only evidence present in the supplied snapshots and the saved structured artifact
- stay bounded to the requested `follow_up_kind`
- treat `active_population_profile`, `population_support_status`, and `fallback_population_profile` as authoritative population scope metadata
- never infer a different population profile or invent unsupported population coverage
- do not upgrade fallback or unavailable claims into stronger language during expansion or challenge
- preserve any supplied `claim_key`, `evidence_tier`, `interpretation_scope`, `active_population_profile`, `population_support_status`, `fallback_population_profile`, and caution metadata when expanding or challenging existing claims
- do not upgrade claim strength beyond the supplied evidence metadata or population support metadata
- only reference general public-health guidance when the underlying claim is guideline-backed and the snapshot provides the anchor
- keep evidence-informed follow-up output contextual and cautious
- keep exploratory follow-up output visibly exploratory, trend-based, or context-only
- expand or challenge claims only with explicit evidence references
- avoid unsupported causal, diagnostic, treatment, or screening language
- recommend only local drill-downs or explicit bounded reruns
- prefer concise, operator-friendly follow-up output over broad essays
- return only the structured follow-up object
