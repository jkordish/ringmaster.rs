# STATUS.md

## Purpose

This file is the current truth for the repository after the navigation, focus, and keyboard-standardization pass landed on `2026-04-11`. It records what shipped, what was verified, and what remains intentionally deferred.

## Navigation and focus truth

The TUI now includes:

- a visible top-level `Views` tab row that remains the primary navigation on wide layouts
- explicit major-region focus state in `AppState`
- a centralized keybinding registry with scoped bindings for global, screen, region, and transient behavior
- one shared keyboard grammar across Dashboard, Timeline, Trends, Explain, Patterns, Review, AI, and Status
- pane-type consistency so selector panes, list panes, chart/pager panes, and detail panes use the same movement and activation rules wherever they appear
- a distinct orientation strip that shows the active screen, focused region, and transient state
- a contextual footer sourced from the registry instead of screen-local hard-coded shortcut copy
- a scoped `?` help overlay
- consistent `Ctrl+F` search behavior for the list-heavy surfaces that support search today
- explicit focus restoration after closing help, closing search, or leaving transient panels
- region-ordered back-out so `Esc` unwinds one screen layer at a time instead of jumping directly to top-level navigation
- visible selection markers that remain distinct from focused-region cues

The navigation-specific documentation added in this pass lives in:

- `docs/HCI_NAVIGATION_RESEARCH.md`
- `docs/NAVIGATION_AUDIT.md`
- `docs/KEYBINDINGS.md`

## Baseline before this pass

Verified before implementation:

- the local-first CLI, sync, derive, review, webhook, UI snapshot, and snapshot/OpenAI artifact flows were already working
- snapshot export already produced canonical hashed artifacts with privacy profiles
- `ai review` and `ai compare` already persisted structured artifacts locally
- reports and eval summaries were already durable and browseable from the CLI and TUI

Primary gaps before this pass:

- `Tab` and `Shift+Tab` switched screens instead of moving between major regions
- `Esc` quit the application instead of canceling or backing out
- visible controls such as trend windows, review tabs, and AI browser tabs did not behave like standard keyboard composites
- footer help was screen-local, dense, and shortcut-first instead of contextual
- focus was not modeled explicitly in app state, so focus restoration and selected-vs-focused cues were inconsistent
- search/find behavior was not standardized across the list-heavy screens

## Current implemented truth

The repository now includes:

- a local snapshot catalog backed by `snapshot_exports`
- an expanded Oura auth capability surface that now tracks `email`, `spo2`, `ring_configuration`, `stress`, and `heart_health` alongside the original baseline scopes
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
  - summary and detail persistence in `ai_eval_runs`
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
  - persisted eval runs
- local jump-back routing from saved AI evidence refs to Review / Explain / Patterns / Timeline when the export ref is resolvable
- richer Ops / doctor summaries for provider readiness, last successful/failed runs, local artifact counts, and eval health

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
- eval fixture manifest summaries, candidate-vs-baseline rollups, failing graders first, and lineage back to saved snapshots, AI runs, and reports when the eval detail payload includes those local handles

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
- persisted manifest/case/grader/linkage detail in `ai_eval_runs.details_json`
- in-app `Evals` browser/detail surface inside the AI workbench
- Status/Ops latest-eval and eval-health diagnostics with warning-state escalation when the newest eval regresses or fails

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
  - `review_prompt_v2`
  - `compare_prompt_v1`
  - `follow_up_prompt_v1`
  - `review_task_frame_v2`
  - `compare_task_frame_v1`
  - `follow_up_task_frame_v1`
- report templates:
  - `markdown_v1`
  - `html_v1`

## Tests now in place

Coverage now includes:

- keybinding registry collision and scope-precedence tests
- reducer tests for region traversal, top-level tab activation, help/search focus restore, and back-out behavior
- deterministic smoke navigation coverage for screen switching, region traversal, search entry, help overlay, and detail return
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
  - snapshot/report/eval browser tabs
  - visible Status eval-health diagnostics
- deterministic eval-browser detail tests for:
  - failing grader rendering
  - linked snapshot / AI run / report lineage lines
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
  - `ui snapshot --screen status --demo`

## Verification run for this pass

Passed in this pass:

- `cargo fmt --all --check`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-nav-ui`

Still blocked after this pass:

- `cargo clippy --all-targets --all-features -- -D warnings`

The remaining clippy failures are not from the navigation architecture changes themselves. They are a broader pre-1.0 lint backlog spread across existing modules such as `src/ai.rs`, `src/lib.rs`, `src/config.rs`, `src/derive.rs`, store/webhook code, and other older surfaces that still trigger strict pedantic/nursery/cargo findings.

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
