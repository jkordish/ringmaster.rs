# STATUS.md

## Purpose

This file is the current truth for the repository during the `snapshot-library-reports-and-eval-flywheel` pass completed on `2026-04-10`. It records what landed, what was verified, and what remains intentionally deferred.

## Baseline before this pass

Verified before implementation:

- the local-first CLI, sync, derive, review, webhook, UI snapshot, and phase-7 snapshot/OpenAI flows were already working
- snapshot export already produced canonical hashed artifacts with privacy profiles
- `ai review` and `ai compare` already persisted structured artifacts locally

Primary gaps before this pass:

- saved snapshots were persisted but not browseable as a first-class local library
- AI review and compare runs were persisted but not easy to inspect over time
- there was no canonical report export command
- prompt framing still lived inline in AI implementation code
- there was no local eval harness for safe prompt/model iteration
- there was no durable report or eval lineage registry

## Current implemented truth

The repository now includes:

- a local snapshot catalog backed by `snapshot_exports`
- canonical snapshot library commands:
  - `snapshot list`
  - `snapshot show`
- a first-class AI run registry backed by `ai_artifacts`
- canonical AI run browse commands:
  - `ai runs list`
  - `ai runs show`
- a canonical report workflow:
  - `report export`
- Markdown and HTML report rendering from either:
  - a saved snapshot
  - a saved AI review run
  - a saved AI compare run
- persisted report manifests in `report_exports`
- a local eval flywheel:
  - `ai eval`
  - fixture manifest support
  - summary persistence in `ai_eval_runs`
  - graders for schema validity, completeness, overclaiming, medical safety, privacy, evidence integrity, and stale-data honesty
- explicit prompt/template/schema versioning with dedicated files under:
  - `src/ai_prompts/*`
  - `src/report_templates/*`
- canonical per-task request builders with stable framing and dry-run request previews
- stronger lineage between snapshots, AI runs, and report exports

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

The catalog stores compact metadata only. It does not leak obvious personal identifiers or secrets in the default redacted path.

## AI artifact capabilities that now work

- `ringmaster ai review <snapshot-path> --dry-run`
- `ringmaster ai compare <snapshot-a> <snapshot-b> --dry-run`
- `ringmaster ai runs list --demo`
- `ringmaster ai runs show <run-id-prefix>`

Persisted AI run metadata now includes:

- artifact kind and status
- provider/model metadata
- prompt version
- output schema version
- request mode and input transport
- privacy profile
- snapshot linkage
- run mode (`real`, `dry_run`, `fixture`)
- request fingerprint
- summary/overview cache for library rendering

## Report export capabilities that now work

- `ringmaster report export --from-snapshot /tmp/ringmaster-snapshot.json --format markdown --out /tmp/ringmaster-report.md`
- `ringmaster report export --from-snapshot /tmp/ringmaster-snapshot.json --format html --out /tmp/ringmaster-report.html`
- `ringmaster report export --from-ai-run <run-id> --format markdown --out /tmp/ringmaster-report.md`

Report exports now include:

- title and source scope
- generation metadata
- freshness and trust summaries
- key findings
- supporting evidence
- sufficiency and uncertainty notes
- provenance references and local lineage handles
- explicit privacy profile and AI usage markers

## Eval flywheel capabilities that now work

- `ringmaster ai eval --fixture-dir tests/fixtures/ai`
- optional JSON summary export via `--export`
- candidate/baseline label selection
- fixture-manifest driven datasets
- deterministic local execution with no live API requirement
- persisted eval summary history in `ai_eval_runs`

The local graders currently cover:

- schema validity
- required-field completeness
- overclaiming / unsupported causality
- medical-advice safety language
- privacy leakage in rendered text
- evidence-reference integrity
- stale or missing data honesty

## Privacy and provenance truth

The current behavior remains intentionally conservative:

- the OpenAI layer is opt-in
- no user data is uploaded unless the user explicitly runs an AI command against a local snapshot
- the exported snapshot remains the only provider-visible boundary object
- `redacted` remains the default export profile
- requests remain stateless by default
- no tools are enabled by default
- local reports and AI runs now show their lineage and privacy profile explicitly
- report/export/eval persistence stores metadata and paths, not hidden background uploads

## Versioning truth

The repository now has explicit version discipline for:

- snapshot schema version: `ringmaster.snapshot.v1`
- review output schema version: `ringmaster.ai.review.v1`
- compare output schema version: `ringmaster.ai.compare.v1`
- prompt templates:
  - `review_prompt_v1`
  - `compare_prompt_v1`
  - `review_task_frame_v1`
  - `compare_task_frame_v1`
- report templates:
  - `markdown_v1`
  - `html_v1`

## Tests now in place

Coverage now includes:

- snapshot catalog tests
- AI run registry tests
- report export tests
- Markdown and HTML renderer tests
- eval harness tests
- prompt-version regression fixtures
- redaction/privacy regression tests
- lineage and provenance persistence tests
- CLI parsing and smoke tests for:
  - `snapshot list`
  - `snapshot show`
  - `ai runs list`
  - `ai runs show`
  - `report export`
  - `ai eval`

## Verification completed for this pass

Verified on `2026-04-10` after implementation:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- snapshot export --demo --profile redacted --out /tmp/ringmaster-snapshot.json`
- `cargo run -- snapshot list --demo`
- `cargo run -- ai review /tmp/ringmaster-snapshot.json --dry-run`
- `cargo run -- ai runs list --demo`
- `cargo run -- report export --from-snapshot /tmp/ringmaster-snapshot.json --format markdown --out /tmp/ringmaster-report.md`
- `cargo run -- ai eval --fixture-dir tests/fixtures/ai`

## Intentionally deferred

- freeform chat or arbitrary natural-language Q&A over the live database
- any direct database-to-OpenAI pipeline
- tool-enabled or browsing-enabled OpenAI runs
- a new AI chat screen in the TUI
- a dedicated TUI artifact browser in this pass
- PDF export as a required format
- batch/archive processing as a user-facing feature
- hosted eval services as a runtime requirement
- packaging, installers, notifications, and release automation
