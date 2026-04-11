# EVALS.md

## Purpose

This document describes the local eval flywheel and in-app eval browser that ship with the snapshot-first AI workflow.

The goal is not to build a hosted eval platform. The goal is to make prompt and model changes safer without changing the local-first privacy posture.

## Canonical command

```bash
cargo run -- ai eval --fixture-dir tests/fixtures/ai
```

Optional flags:

- `--candidate <label>` to select a specific candidate artifact label
- `--baseline <label>` to select a specific baseline artifact label
- `--export <path>` to write a JSON summary file

## What `ai eval` does

`ai eval` loads a local fixture manifest, validates the snapshot fixtures, loads candidate and baseline AI artifacts, runs local graders, and writes a compact summary to stdout.

It also persists an `ai_eval_runs` row so regressions can be inspected over time.

That persisted row now includes:

- rollup scores and regression summary fields for CLI and Ops summaries
- a `details_json` payload with fixture-manifest metadata, per-case outcomes, per-grader comparisons, and optional lineage back to saved snapshots, AI runs, artifacts, and reports
- enough detail for the TUI `AI -> Evals` browser to stay read-only and deterministic without rerunning fixture directories

## Fixture layout

The eval harness is manifest-driven.

Current fixture root:

```text
tests/fixtures/ai/
  manifest.json
  review-snapshot.json
  review-candidate.json
  review-baseline.json
  review-sparse-snapshot.json
  review-sparse-candidate.json
  review-sparse-baseline.json
  compare-snapshot-a.json
  compare-snapshot-b.json
  compare-candidate.json
  compare-baseline.json
```

The manifest declares:

- schema version
- default candidate label
- default baseline label
- each case id
- task family (`review` or `compare`)
- snapshot fixture paths
- artifact fixture paths
- case expectations
- optional lineage metadata for candidate and baseline artifacts
- optional saved snapshot hashes for snapshot A / snapshot B linkage

## Graders

The current harness runs these graders:

- `schema_validity`
- `completeness`
- `required_content`
- `distinct_finding_titles`
- `overclaiming`
- `medical_safety`
- `privacy`
- `evidence`
- `follow_up_targets`
- `honesty`

### Schema validity

Checks that the structured artifact parses into the expected typed artifact shape for the task family.

### Completeness

Checks that the artifact produces at least the expected number of primary findings and, where declared, the expected primary title.

### Required content

Checks for explicit required caveat text, such as single-day or sync-failure acknowledgements, when a fixture expects those limits to be called out directly.

### Distinct finding titles

Checks that review artifacts do not repeat the same finding title across headline, positive, and negative sections when the fixture requires de-duplication.

### Overclaiming

Flags obviously unsupported causal or certainty language in rendered output.

### Medical safety

Flags wording that crosses into medical advice instead of bounded product interpretation.

### Privacy

Checks for forbidden substrings such as account identifiers, tokens, or secrets in rendered output.

### Evidence integrity

Checks that evidence references point to export refs that actually exist in the fixture snapshots.

### Follow-up targets

Checks that artifact drill-down commands match the deterministic local expectations encoded in the fixture manifest.

### Stale-data honesty

Checks that artifacts remain candid about stale or missing data when the fixture requires honesty.

## Reproducibility rules

The local eval loop is designed to stay reproducible:

- snapshot fixtures are canonical boundary objects
- no live API calls are required for baseline regression coverage
- candidate and baseline artifacts are stored locally
- the eval summary is stable for the same fixtures and grader logic
- per-case artifacts can be exported explicitly, but are not required to be persisted by default

## Prompt and model comparisons

The fixture manifest plus candidate and baseline labels are the local comparison mechanism.

In practice, this means:

- prompt changes should land with updated fixtures or expected outcomes
- model changes can be checked against the same fixture set
- prompt/schema/model/provider versions remain visible in both AI run records and eval summaries

## In-app eval browser

The top-level `AI` workbench now includes an `Evals` tab.

Selecting a persisted eval run shows:

- fixture manifest summary
- candidate vs baseline rollup
- score rollups and regression/improvement summaries
- failing graders first, rendered natively instead of dumped as raw JSON
- linked snapshots, AI runs, artifacts, and reports when the manifest declared resolvable lineage metadata

The `Status` screen also surfaces:

- the latest eval timestamp and candidate/baseline labels
- eval health counts for failed cases, regressions, and improvements
- a warning when the newest eval needs attention

## Why this is local-first

The eval workflow intentionally does not depend on:

- a hosted eval service
- background cloud processing
- a direct database-to-model pipeline
- freeform chat

That keeps the reliability loop inspectable, deterministic, and compatible with the same privacy posture as the rest of the app.

## Current limitations

The current eval harness does not yet provide:

- batch archive jobs over very large saved snapshot collections
- PDF report output
- automatic fixture generation from live runs
- interactive per-case cursoring inside the TUI detail pane

Those are intentionally deferred until the local artifact workflows settle further.
