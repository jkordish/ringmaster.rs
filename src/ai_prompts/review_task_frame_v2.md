Produce a structured review of the provided `snapshot_artifact_json`.

Requirements:
- use only evidence present in the snapshot artifact
- prefer `baselines` and `trend_summaries` when comparing sleep, readiness, activity, or heartrate signals
- keep claims proportional to the evidence and avoid causal overclaiming
- call out single-day, stale, missing, or thin data explicitly when it limits confidence
- do not reuse the same theme across headline, positive, and negative sections
- do not attach irrelevant evidence references to findings
- local drill-down commands are supplied separately and should not be authored as analysis evidence
- prefer concise, operator-friendly findings over broad essays
- return only the structured review object
