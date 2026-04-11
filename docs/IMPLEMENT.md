# IMPLEMENT.md

## Purpose

This file is the execution runbook for the current product. It only describes flows that work today, including the standardized TUI navigation model, the AI workbench, inline AI launch points, preflight confirmation, the snapshot library, AI run registry, report export workflow, the in-app eval browser, and the local eval harness.

## Commands

Current commands:

```bash
cargo run -- tui
cargo run -- tui --demo
cargo run -- demo
cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-ui-snapshots
cargo run -- ui snapshot --screen ai --demo --out-dir /tmp/ringmaster-ai-ui
cargo run -- ui snapshot --screen status --demo --out-dir /tmp/ringmaster-status-ui
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
cargo run -- report export --from-ai-run <artifact-id-or-prefix> --format html --out /tmp/ringmaster-report.html
cargo run -- ai eval --fixture-dir tests/fixtures/ai
cargo run -- webhook serve
cargo run -- webhook replay --fixture tests/fixtures/webhooks/sample.json
cargo run -- webhook subscriptions list --fixture-dir tests/fixtures/webhooks
cargo run -- webhook subscriptions sync --dry-run --fixture-dir tests/fixtures/webhooks
```

`auth login` now requests the broader current Oura scope set by default and the product surfaces the result explicitly in `doctor`, auth status, and the TUI ops/auth readouts. Scopes that are granted but not yet wired into local sync, such as `spo2` and `ring_configuration`, are shown as future-ready instead of being silently ignored.

## TUI navigation runtime

The interactive TUI now uses one shared navigation model across every primary screen.

Region model:

1. `Views` top navigation
2. screen-local controls such as trend windows, review mode/focus tabs, or AI browser tabs when the screen has a real selector pane
3. primary working region such as a list, launch-point group, chart, or main content pane
4. secondary or detail regions when the screen has a real drill-down or independently operable companion pane

Keyboard rules:

- `Tab` / `Shift+Tab`: move between major regions
- arrows, `Home`, `End`, `PageUp`, `PageDown`: move within the focused composite according to pane type
- `Enter` / `Space`: activate or commit
- `Esc`: close help, close search, dismiss a transient panel, or back out one interaction layer
- `Ctrl+F`: open search in the current searchable context
- `?`: open the scoped keyboard-help overlay

Pane-type rules:

- selector panes such as `Views`, Timeline window presets, Timeline / Explain / Patterns overlay selectors, trend windows, the Patterns metric filter, review mode/focus, AI browser tabs, and AI preflight controls use `Left` / `Right` for one-step moves and `Home` / `End` or `PageUp` / `PageDown` for edge jumps
- list panes such as Timeline events, Review cards, AI launch points, saved artifacts, and AI artifact actions use `Up` / `Down`, `Home` / `End`, and `PageUp` / `PageDown`
- chart/pager panes such as the Timeline chart use `Left` / `Right` within the active day and `PageUp` / `PageDown` for larger day shifts
- detail panes are explicit drill-down surfaces; `Enter` / `Space` and `Esc` return to the invoking region instead of inventing screen-specific navigation keys

Back-out is now consistent with screen region order. `Esc` walks to the previous major region on the current screen before it returns to `Views`.

Read-mostly screens do not manufacture extra focus stops. If a visible section does not have its own movement or activation contract, it stays inside the screen's body region instead of becoming a separate keyboard destination.

Search works today on the current list-heavy surfaces:

- Timeline event list
- Review ranked observations
- AI saved-artifact browser list

The footer is registry-driven and intentionally concise. The help overlay is the place for the full scoped command list, including optional expert aliases.

Canonical promoted controls now include:

- Timeline `Window presets` plus `Overlay filters`
- Explain `Overlay filters`
- Patterns `Metric filter` plus `Family filter`
- AI `Artifact actions`
- AI preflight `Controls`

Navigation regression coverage now lives in the app, keybinding, and TUI test suites plus the deterministic `ui snapshot --demo` artifact pass.

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

## AI workbench runtime

The TUI now exposes AI as a first-class product surface instead of treating it as a CLI-only add-on.

Top-level entry:

- `AI` screen in the TUI

Inline launch points exist from:

- `Dashboard`
- `Review`
- `Explain`
- `Patterns`
- saved snapshot / AI run / report detail slices inside the AI browser

The workbench is intentionally guided rather than conversational:

- no freeform chat textbox
- no arbitrary prompt composition
- no hidden uploads
- no live database inspection by the model

The in-app flow is:

1. open the AI workbench directly or route there from an inline launch point
2. select a bounded launch or inspect a saved artifact and move into `Artifact actions`
3. inspect the preflight panel and its visible `Controls` row before any provider call
4. move between preflight controls with `Tab` / `Shift+Tab` or `Left` / `Right`, then confirm with `Enter` / `Space` or cancel with `Esc`
5. monitor the persisted run lifecycle in-app
6. inspect the saved structured result, linked reports, and source snapshot lineage
7. jump back to local evidence screens or export a report

## AI review, compare, follow-up, and registry runtime

`ringmaster ai review` and `ringmaster ai compare` only accept local snapshot files as input. The TUI workbench uses the same provider boundary after first generating or resolving the exact local snapshot artifacts that will be sent.

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

### Guided follow-up behavior

Saved AI runs can launch bounded follow-up actions from the AI workbench:

- expand evidence
- show strongest counterevidence
- explain ranking
- suggest next local drill-down
- compare against a previous similar snapshot
- rerun with a different privacy profile
- rerun with a different model
- generate a report from the selected saved artifact

Model-backed follow-up actions stay schema-bound and snapshot-bounded. They do not open an arbitrary prompt surface.
The visible `Artifact actions` pane is the canonical path for these workflows. Letter bindings such as `e`, `g`, `u`, and `m` remain optional expert aliases only.

### Output handling

Review, compare, and model-backed follow-up commands:

1. validate the snapshot input
2. run the selected provider
3. serialize the structured artifact to JSON
4. render the human-readable briefing locally
5. persist an `ai_artifacts` row with snapshot linkage, provider metadata, versions, and summary cache
6. persist or update an `ai_runs` row with lifecycle state, request preview, model/provider provenance, linkage, and any failure metadata
6. optionally write the raw JSON artifact to disk

Saved AI runs are browseable through:

- `ai runs list`
- `ai runs show <run-id>`
- the TUI `AI` workbench browser tabs for snapshots, runs, and reports

Persisted run states:

- `queued`
- `running`
- `succeeded`
- `failed`
- `cancelled`
- `interrupted`

Runs interrupted by a previous process are marked locally during startup so the browser does not leave stale in-flight state behind.

## Preflight confirmation

Every in-app AI launch routes through a compact preflight panel before any provider request leaves the machine.

The preflight view shows:

- snapshot scope
- privacy profile
- provider and model
- request mode
- stateless status
- tool-disabled status
- exact artifact path(s)
- content classes included in the payload
- whether notes or free text are included
- approximate payload size and token estimate
- any warnings such as disabled providers or recent failures
- a visible `Controls` row for confirm, rotate privacy, and cancel

The user must confirm explicitly. Dismissing preflight performs no hidden network action.

## Artifact browser runtime

The AI workbench browser is now the canonical in-app artifact surface for:

- snapshots
- AI runs
- exported reports
- persisted eval runs

The browser uses a consistent list/detail model and keeps provenance visible:

- snapshot scope and privacy profile
- AI provider/model/request metadata
- prompt and schema versions
- linked artifacts and linked reports
- exported report output path and verification metadata
- eval fixture manifests, baseline-vs-candidate summaries, and failing graders
- a visible `Artifact actions` list that changes truthfully with the selected run, snapshot, report, or eval

## Report export runtime

`ringmaster report export` is the canonical human-facing report workflow.

Supported sources:

- `--from-snapshot <snapshot-id-or-path>`
- `--from-ai-run <artifact-id-or-unique-prefix>`

Supported formats:

- `markdown`
- `html`

Current behavior:

1. resolve the source snapshot or saved AI artifact
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

The same report export flow is available from the AI workbench through the visible `Generate report` artifact action on eligible saved runs and snapshots. The `g` binding remains an expert alias.

## Eval runtime

`ringmaster ai eval --fixture-dir <dir>` is the canonical local reliability loop.

Runtime behavior:

1. load the fixture manifest from the fixture directory
2. load snapshot fixtures and candidate/baseline artifact fixtures
3. validate snapshot integrity and schema versions
4. run local graders against rendered artifact text and evidence refs
5. render a compact summary to stdout
6. optionally export a JSON summary file
7. persist an `ai_eval_runs` row with both rollup metrics and a `details_json` payload for the in-app eval browser

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
- eval persistence stays local and snapshot-first
- historical eval browsing reads persisted detail payloads instead of rerunning fixtures from the TUI
- fixture lineage metadata is optional and is only used when the manifest declares explicit local handles

## Prompt, schema, and template versioning

Prompt and rendering assets now live in explicit versioned files:

- `src/ai_prompts/review_prompt_v2.md`
- `src/ai_prompts/compare_prompt_v1.md`
- `src/ai_prompts/review_task_frame_v2.md`
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
- `ai_runs`
- `report_exports`
- `ai_eval_runs`

These tables are used for:

- snapshot library browsing
- local evidence-to-record mapping
- saved AI review/compare artifacts
- persisted run lifecycle and request preview inspection
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

There is intentionally no freeform AI chat surface in this pass. The AI workbench and artifact browser are guided, structured, and provenance-driven instead of chat-driven.

## `ui snapshot`

`ringmaster ui snapshot` remains the canonical non-interactive design-QA surface. It uses the same rendering stack as the interactive TUI and writes deterministic UTF-8 artifacts to disk.

Supported sources:

- `--demo` for deterministic built-in presentation data
- `--fixture-dir <dir>` for a fixture-backed temporary store
- live local store when neither `--demo` nor `--fixture-dir` is passed

Important current usage:

- `--screen ai` renders deterministic AI workbench snapshots
- demo AI snapshots cover provider-disabled, preflight, running, success, failure/cancel, and saved-detail paths
- `--screen status` renders deterministic Status/Ops snapshots, including eval-health diagnostics

## Verification sequence

Use this order unless a narrower check is sufficient while developing:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo run -- doctor
cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-nav-ui
```

## Notes for future passes

- keep the AI boundary snapshot-first
- keep the TUI pure even though it now launches AI runs and report exports through actions handled outside widgets
- do not add freeform chat without a dedicated design pass
- do not enable tools, browsing, or direct database inspection in the provider path without a new privacy review
- keep guided follow-up actions schema-bound instead of becoming generic prompting
