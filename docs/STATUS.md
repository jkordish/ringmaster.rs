# STATUS.md

## Purpose

This file is the current truth for the repository after the `ai-workbench-and-first-class-tui` pass completed on `2026-04-10`. It records what landed, what was verified, and what remains intentionally deferred.

## Baseline before this pass

Verified before implementation:

- the local-first CLI, sync, derive, review, webhook, UI snapshot, and phase-8 snapshot/OpenAI flows were already working
- snapshot export already produced canonical hashed artifacts with privacy profiles
- `ai review` and `ai compare` already persisted structured artifacts locally
- reports and eval summaries were already durable and browseable from the CLI

Primary gaps before this pass:

- AI still felt CLI-first even though snapshots, AI artifacts, reports, and evals were already durable
- there was no dedicated in-app AI workbench
- there was no explicit preflight confirmation gate inside the TUI
- saved snapshots, AI runs, and reports were not browseable together in-app
- guided follow-up actions were not available from the TUI
- the ops surface did not clearly expose AI/provider readiness and usage

## Current implemented truth

The repository now includes:

- a local snapshot catalog backed by `snapshot_exports`
- canonical snapshot library commands:
  - `snapshot list`
  - `snapshot show`
- a durable AI artifact registry backed by `ai_artifacts`
- a durable AI run lifecycle registry backed by `ai_runs`
- canonical AI run browse commands:
  - `ai runs list`
  - `ai runs show`
- a canonical report workflow:
  - `report export`
- Markdown and HTML report rendering from either:
  - a saved snapshot
  - a saved AI artifact id / unique prefix from the local registry
- persisted report manifests in `report_exports`
- a local eval flywheel:
  - `ai eval`
  - fixture manifest support
  - summary persistence in `ai_eval_runs`
  - graders for schema validity, completeness, overclaiming, medical safety, privacy, evidence integrity, and stale-data honesty
- explicit prompt/template/schema versioning with dedicated files under:
  - `src/ai_prompts/*`
  - `src/report_templates/*`
- canonical per-task request builders with stable framing and typed request previews
- stronger lineage between snapshots, AI runs, report exports, and guided follow-up outputs
- a dedicated top-level `AI` workbench screen in the TUI
- inline AI launch points from `Dashboard`, `Explain`, `Patterns`, `Review`, and the workbench itself
- an explicit preflight panel that shows:
  - snapshot scope
  - privacy profile
  - provider/model
  - request mode
  - stateless mode
  - tools-disabled status
  - content classes
  - approximate payload size / token estimate
  - exact local artifact path
- in-app async AI run orchestration with visible lifecycle states:
  - `queued`
  - `running`
  - `succeeded`
  - `failed`
  - `cancelled`
  - `interrupted`
- guided follow-up actions inside the workbench for:
  - expand evidence
  - show strongest counterevidence
  - explain ranking
  - suggest next local drill-down
  - rerun with another privacy profile
  - rerun with another model
  - compare against a previous similar snapshot
  - generate report
- in-app browsing and inspection for:
  - snapshots
  - AI runs
  - exported reports
- local jump-back routing from saved AI evidence refs to Review / Explain / Patterns / Timeline when the export ref is resolvable
- richer Ops / doctor summaries for provider readiness, last successful/failed runs, and local artifact counts

## Snapshot library capabilities that now work

- `ringmaster snapshot export --demo --profile redacted --out /tmp/ringmaster-snapshot.json`
- `ringmaster snapshot list --demo`
- `ringmaster snapshot show /tmp/ringmaster-snapshot.json`
- `ringmaster snapshot show <snapshot-hash-prefix>`

Snapshot catalog metadata now exposes:

- created time
- stable snapshot hash identity
- scope and day bounds
- privacy profile
- schema version
- source mode
- freshness summary
- trust summary
- capability summary
- provenance summary
- linked run/report counts in the AI workbench browser

## AI artifact and workbench capabilities that now work

- `ringmaster ai review <snapshot-path> --dry-run`
- `ringmaster ai compare <snapshot-a> <snapshot-b> --dry-run`
- `ringmaster ai runs list --demo`
- `ringmaster ai runs show <run-id>`
- `ringmaster ui snapshot --screen ai --demo --out-dir /tmp/ringmaster-ai-ui`

Persisted AI run metadata now includes:

- run lifecycle status
- run kind
- provider/model metadata
- prompt version
- output schema version
- request mode and input transport
- privacy profile
- snapshot linkage
- request fingerprint
- typed request preview payload
- linked source artifact for follow-up runs where applicable
- linked report exports through the browser detail panel

The Review screen now shows, for the currently selected day:

- whether a saved AI artifact exists
- whether the latest saved run was a `review` or `compare`
- compact saved summary text derived from local `summary_cache` and `overview`
- provenance-first lineage including run id, matched snapshot hash, compare peer hash when present, provider/model, prompt version, schema version, and privacy profile

The AI workbench now shows, for the selected browser item:

- persisted request preview metadata
- structured findings, evidence refs, counterevidence refs, unresolved questions, and guided follow-up targets
- linkage to the source snapshot(s)
- linkage to exported reports
- error detail for failed/cancelled/interrupted runs

## Report export capabilities that now work

- `ringmaster report export --from-snapshot /tmp/ringmaster-snapshot.json --format markdown --out /tmp/ringmaster-report.md`
- `ringmaster report export --from-snapshot /tmp/ringmaster-snapshot.json --format html --out /tmp/ringmaster-report.html`
- `ringmaster report export --from-ai-run <artifact-id-or-prefix> --format markdown --out /tmp/ringmaster-report.md`

Report exports now include:

- title and source scope
- generation metadata
- freshness and trust summaries
- key findings
- supporting evidence
- sufficiency and uncertainty notes
- provenance references and local lineage handles
- explicit privacy profile and AI usage markers
- follow-up artifact summaries when the source is a saved guided follow-up artifact

## Eval flywheel capabilities that now work

- `ringmaster ai eval --fixture-dir tests/fixtures/ai`
- optional JSON summary export via `--export`
- candidate/baseline label selection
- fixture-manifest driven datasets
- deterministic local execution with no live API requirement
- persisted eval summary history in `ai_eval_runs`

## Privacy and provenance truth

The current behavior remains intentionally conservative:

- the OpenAI layer is opt-in
- no TUI action uploads anything until the user explicitly confirms preflight
- no user data is uploaded unless the user explicitly runs an AI command against a local snapshot
- the exported snapshot remains the only provider-visible boundary object
- `redacted` remains the default export profile
- requests remain stateless by default
- no tools are enabled by default
- local reports, AI runs, and the preflight panel show lineage and privacy profile explicitly
- report/export/eval persistence stores metadata and paths, not hidden background uploads

## Versioning truth

The repository now has explicit version discipline for:

- snapshot schema version: `ringmaster.snapshot.v1`
- review output schema version: `ringmaster.ai.review.v1`
- compare output schema version: `ringmaster.ai.compare.v1`
- follow-up output schema version: `ringmaster.ai.follow_up.v1`
- prompt templates:
  - `review_prompt_v1`
  - `compare_prompt_v1`
  - `follow_up_prompt_v1`
  - `review_task_frame_v1`
  - `compare_task_frame_v1`
  - `follow_up_task_frame_v1`
- report templates:
  - `markdown_v1`
  - `html_v1`

## Tests now in place

Coverage now includes:

- snapshot catalog tests
- AI artifact and AI run registry tests
- report export tests
- Markdown and HTML renderer tests
- eval harness tests
- prompt-version regression fixtures
- redaction/privacy regression tests
- lineage and provenance persistence tests
- TestBackend / buffer-based AI workbench tests for:
  - provider-disabled workbench rendering
  - preflight overlay rendering
  - saved-run detail rendering
  - snapshot/report browser tabs
- deterministic lifecycle/model tests for:
  - queued
  - running
  - succeeded
  - failed
  - cancelled AI runs
- CLI parsing and smoke tests for:
  - `snapshot list`
  - `snapshot show`
  - `ai runs list`
  - `ai runs show`
  - `report export`
  - `ai eval`
  - `ui snapshot --screen ai --demo`

## Verification completed for this pass

The following commands were run and passed for this pass:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- snapshot export --demo --profile redacted --out /tmp/ringmaster-snapshot.json`
- `cargo run -- ai review /tmp/ringmaster-snapshot.json --dry-run`
- `cargo run -- ui snapshot --screen ai --demo --out-dir /tmp/ringmaster-ai-ui`
- `cargo test --lib 'tui::tests::ai_workbench_smoke_path_covers_disabled_preflight_and_saved_run_detail' -- --exact`

## Intentionally deferred

- freeform chat or arbitrary natural-language Q&A over the live database
- any direct database-to-OpenAI pipeline
- tool-enabled or browsing-enabled OpenAI runs
- arbitrary prompt composition in the TUI
- automatic background AI runs
- PDF export as a required format
- batch/archive processing as a user-facing feature
- hosted eval services as a runtime requirement
- packaging, installers, notifications, and release automation
