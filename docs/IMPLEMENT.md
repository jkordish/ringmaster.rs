# IMPLEMENT.md

## Purpose

This file is the execution runbook for the current phase-8 product. It only describes flows that work today, including the snapshot library, AI run registry, report export workflow, and local eval harness.

## Commands

Current commands:

```bash
cargo run -- tui
cargo run -- tui --demo
cargo run -- demo
cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-ui-snapshots
cargo run -- doctor
cargo run -- auth login
cargo run -- sync once
cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase3
cargo run -- sync watch
cargo run -- sync watch --demo --max-iterations 1
cargo run -- derive rebuild
cargo run -- derive rebuild --demo
cargo run -- review today --demo
cargo run -- review week --demo
cargo run -- review investigate --focus readiness --demo
cargo run -- snapshot export --demo --profile redacted --out /tmp/ringmaster-snapshot.json
cargo run -- snapshot list --demo
cargo run -- snapshot show /tmp/ringmaster-snapshot.json
cargo run -- ai review /tmp/ringmaster-snapshot.json --dry-run
cargo run -- ai compare /tmp/ringmaster-snapshot.json /tmp/ringmaster-snapshot.json --dry-run
cargo run -- ai runs list --demo
cargo run -- ai runs show <run-id>
cargo run -- report export --from-snapshot /tmp/ringmaster-snapshot.json --format markdown --out /tmp/ringmaster-report.md
cargo run -- report export --from-ai-run <run-id> --format html --out /tmp/ringmaster-report.html
cargo run -- ai eval --fixture-dir tests/fixtures/ai
cargo run -- webhook serve
cargo run -- webhook replay --fixture tests/fixtures/webhooks/sample.json
cargo run -- webhook subscriptions list --fixture-dir tests/fixtures/webhooks
cargo run -- webhook subscriptions sync --dry-run --fixture-dir tests/fixtures/webhooks
```

## Snapshot library runtime

`ringmaster snapshot export` remains the canonical boundary between local product state and any optional external AI analysis.

Runtime behavior:

1. resolve a bounded scope from the local store
2. load auth capability metadata, persisted sync state, typed metrics, and derived review/context/pattern artifacts
3. apply the selected privacy profile
4. build a `SnapshotBundleV1`
5. compute a deterministic snapshot hash
6. persist the snapshot manifest and local provenance refs
7. render pretty or compact JSON to stdout or disk

The snapshot library is then browseable through:

- `snapshot list`
- `snapshot show <snapshot-id-or-path>`

Important rules:

- the snapshot is useful even when AI is disabled
- the snapshot is the only thing the optional OpenAI provider can inspect
- exported provenance refs stay opaque and local-first
- snapshot export never includes auth secrets or raw config internals
- catalog summaries stay compact and avoid secret leakage

### Privacy profiles

Implemented profiles:

- `redacted`
  - default
  - strips obvious account identifiers
  - removes free-text review signal payloads
  - minimizes text leakage while keeping metrics, trends, and follow-up references useful
- `balanced`
  - keeps the same core redaction posture while allowing richer derived labels where safe
- `full`
  - preserves more local explanatory text, but still excludes secrets and config/auth internals

## AI review, compare, and registry runtime

`ringmaster ai review` and `ringmaster ai compare` only accept local snapshot files as input.

Provider selection order:

1. `--dry-run`
2. `--fixture <path>`
3. configured real provider

### Dry-run behavior

- no network request
- deterministic structured artifacts generated locally
- request preview output shows the final request shape, version metadata, and request fingerprint
- useful for smoke tests and CI

### Fixture behavior

- no network request
- loads a stored JSON artifact from disk
- useful for prompt regression and rendering tests

### Real OpenAI behavior

- uses the Responses API
- uses strict Structured Outputs via JSON Schema
- sends no tools in this pass
- defaults to stateless mode
- persists the returned artifact locally after local validation

### Output handling

Both commands:

1. validate the snapshot input
2. run the selected provider
3. serialize the structured artifact to JSON
4. render the human-readable briefing locally
5. persist an `ai_artifacts` row with snapshot linkage, provider metadata, versions, and summary cache
6. optionally write the raw JSON artifact to disk

Saved AI runs are browseable through:

- `ai runs list`
- `ai runs show <run-id>`

## Report export runtime

`ringmaster report export` is the canonical human-facing report workflow.

Supported sources:

- `--from-snapshot <snapshot-id-or-path>`
- `--from-ai-run <run-id>`

Supported formats:

- `markdown`
- `html`

Current behavior:

1. resolve the source snapshot or AI run
2. auto-catalog linked snapshots when needed so lineage remains valid
3. build a shared `ReportDocument` view model
4. render the requested format
5. write the output file
6. persist a `report_exports` manifest row

Each report includes:

- title and scope
- generation metadata
- freshness and trust summary
- key findings
- supporting evidence
- uncertainty and sufficiency notes
- provenance references
- explicit privacy profile and AI usage marker

## Eval runtime

`ringmaster ai eval --fixture-dir <dir>` is the canonical local reliability loop.

Runtime behavior:

1. load the fixture manifest from the fixture directory
2. load snapshot fixtures and candidate/baseline artifact fixtures
3. validate snapshot integrity and schema versions
4. run local graders against rendered artifact text and evidence refs
5. render a compact summary to stdout
6. optionally export a JSON summary file
7. persist an `ai_eval_runs` summary row

Current graders:

- schema validity
- required-field completeness
- overclaiming
- medical safety language
- privacy leakage
- evidence-reference integrity
- stale-data honesty

Important rules:

- no live API calls are required for baseline regression coverage
- fixture directories are deterministic and suitable for CI
- eval persistence stores summaries, not every case payload by default

## Prompt, schema, and template versioning

Prompt and rendering assets now live in explicit versioned files:

- `src/ai_prompts/review_prompt_v1.md`
- `src/ai_prompts/compare_prompt_v1.md`
- `src/ai_prompts/review_task_frame_v1.md`
- `src/ai_prompts/compare_task_frame_v1.md`
- `src/report_templates/markdown_v1.md`
- `src/report_templates/html_v1.html`

Persisted AI runs record:

- prompt version
- output schema version
- provider/model
- request mode
- input transport
- run mode
- request fingerprint

## Persistence and schema

Current snapshot/AI/report/eval persistence tables:

- `snapshot_exports`
- `snapshot_provenance_refs`
- `ai_artifacts`
- `report_exports`
- `ai_eval_runs`

These tables are used for:

- snapshot library browsing
- local evidence-to-record mapping
- saved AI review/compare artifacts
- report export lineage
- eval run history
- prompt/schema/provider drift diagnosis

## Visual-system runtime

The TUI still uses the dedicated shared presentation layer under `src/ui/*`.

Important boundary:

- widgets never perform HTTP
- widgets never refresh tokens
- widgets never write to SQLite
- widgets never invoke the OpenAI provider directly
- the render path stays on persisted presentation models only

There is intentionally no freeform AI chat surface and no artifact browser screen in the TUI in this pass.

## `ui snapshot`

`ringmaster ui snapshot` remains the canonical non-interactive design-QA surface. It uses the same rendering stack as the interactive TUI and writes deterministic UTF-8 artifacts to disk.

Supported sources:

- `--demo` for deterministic built-in presentation data
- `--fixture-dir <dir>` for a fixture-backed temporary store
- live local store when neither `--demo` nor `--fixture-dir` is passed

## Verification sequence

Use this order unless a narrower check is sufficient while developing:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo run -- doctor
cargo run -- snapshot export --demo --profile redacted --out /tmp/ringmaster-snapshot.json
cargo run -- snapshot list --demo
cargo run -- ai review /tmp/ringmaster-snapshot.json --dry-run
cargo run -- ai runs list --demo
cargo run -- report export --from-snapshot /tmp/ringmaster-snapshot.json --format markdown --out /tmp/ringmaster-report.md
cargo run -- ai eval --fixture-dir tests/fixtures/ai
```

## Notes for future passes

- keep the AI boundary snapshot-first
- keep the TUI pure and read-only with respect to saved AI artifacts
- do not add freeform chat without a dedicated design pass
- do not enable tools, browsing, or direct database inspection in the provider path without a new privacy review
- add a read-only TUI artifact browser only if it can reuse the same report and registry rendering models
