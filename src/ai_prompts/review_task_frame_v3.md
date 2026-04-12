Produce a structured review of the provided `snapshot_artifact_json`.

Requirements:
- use only evidence present in the snapshot artifact
- treat `active_population_profile`, `population_support_status`, and `fallback_population_profile` as authoritative population scope metadata
- never infer a different population profile or invent unsupported population coverage
- for `general_adult_only_fallback`, say that the guidance is general-adult-only and not matched to the active population
- for `unavailable`, avoid stronger interpretive language and explicitly describe the lack of population-specific support
- prefer `baselines`, `trend_summaries`, `review_signals`, and attached evidence metadata when comparing sleep, activity, readiness, or heart-rate-related signals
- populate `claim_key`, `evidence_tier`, `interpretation_scope`, `active_population_profile`, `population_support_status`, `fallback_population_profile`, and `caution_labels` whenever the snapshot supports them
- do not upgrade claim strength beyond the supplied evidence metadata or population support metadata
- guideline-backed language may reference general public-health guidance only when the snapshot provides the matching anchor
- evidence-informed findings must remain cautious and contextual
- exploratory findings must stay visibly exploratory, trend-based, or context-only
- keep claims proportional to the evidence and avoid causal, screening, diagnostic, or treatment overreach
- call out single-day, stale, missing, thin, or device-limited data explicitly when it limits confidence
- preserve sensitive-metric limitations for SpO2 and consumer sleep technology outputs
- do not reuse the same theme across headline, positive, and negative sections
- do not attach irrelevant evidence references to findings
- local drill-down commands are supplied separately and should not be authored as analysis evidence
- prefer concise, operator-friendly findings over broad essays
- return only the structured review object
