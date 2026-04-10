# EVALS.md

## Purpose

This document describes the local eval flywheel that ships with the Phase 8 snapshot library and report workflow.

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

It also persists an `ai_eval_runs` summary row so regressions can be inspected over time.

## Fixture layout

The eval harness is manifest-driven.

Current fixture root:

```text
tests/fixtures/ai/
  manifest.json
  review-snapshot.json
  review-candidate.json
  review-baseline.json
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

## Graders

The current harness runs these graders:

- `schema_validity`
- `completeness`
- `overclaiming`
- `medical_safety`
- `privacy`
- `evidence`
- `honesty`

### Schema validity

Checks that the structured artifact parses into the expected typed artifact shape for the task family.

### Completeness

Checks that the artifact produces at least the expected number of primary findings and, where declared, the expected primary title.

### Overclaiming

Flags obviously unsupported causal or certainty language in rendered output.

### Medical safety

Flags wording that crosses into medical advice instead of bounded product interpretation.

### Privacy

Checks for forbidden substrings such as account identifiers, tokens, or secrets in rendered output.

### Evidence integrity

Checks that evidence references point to export refs that actually exist in the fixture snapshots.

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

## Why this is local-first

The eval workflow intentionally does not depend on:

- a hosted eval service
- background cloud processing
- a direct database-to-model pipeline
- freeform chat

That keeps the reliability loop inspectable, deterministic, and compatible with the same privacy posture as the rest of the app.

## Current limitations

The current eval harness does not yet provide:

- a dedicated TUI eval browser
- batch archive jobs over very large saved snapshot collections
- PDF report output
- automatic fixture generation from live runs

Those are intentionally deferred until the local artifact workflows settle further.
