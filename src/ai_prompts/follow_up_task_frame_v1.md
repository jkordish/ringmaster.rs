Produce a structured follow-up for the provided `source_artifact_json` using the supplied snapshot artifact JSON.

Requirements:
- use only evidence present in the supplied snapshots and the saved structured artifact
- stay bounded to the requested `follow_up_kind`
- expand or challenge claims only with explicit evidence references
- prefer concise, operator-friendly follow-up output over broad essays
- recommend only local drill-downs or explicit bounded reruns
- return only the structured follow-up object
