# IMPLEMENT.md

## Purpose

This file is the execution runbook for the current phase-7 product. It only describes flows that work today, including the snapshot-first optional OpenAI layer.

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
cargo run -- ai review /tmp/ringmaster-snapshot.json --dry-run
cargo run -- ai compare /tmp/ringmaster-snapshot.json /tmp/ringmaster-snapshot.json --dry-run
cargo run -- webhook serve
cargo run -- webhook replay --fixture tests/fixtures/webhooks/sample.json
cargo run -- webhook subscriptions list --fixture-dir tests/fixtures/webhooks
cargo run -- webhook subscriptions sync --dry-run --fixture-dir tests/fixtures/webhooks
```

## Snapshot export runtime

`ringmaster snapshot export` is the canonical boundary between local product state and any optional external AI analysis.

Runtime behavior:

1. resolve a bounded scope from the local store
2. load auth capability metadata, persisted sync state, typed metrics, and derived review/context/pattern artifacts
3. apply the selected privacy profile
4. build a `SnapshotBundleV1`
5. compute a deterministic snapshot hash
6. persist the snapshot manifest and local provenance refs
7. render pretty or compact JSON to stdout or disk

Important rules:

- the snapshot is useful even when AI is disabled
- the snapshot is the only thing the optional OpenAI provider can inspect
- exported provenance refs stay opaque and local-first
- snapshot export never includes auth secrets or raw config internals

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

## AI review and compare runtime

`ringmaster ai review` and `ringmaster ai compare` only accept local snapshot files as input.

Provider selection order:

1. `--dry-run`
2. `--fixture <path>`
3. configured real provider

### Dry-run behavior

- no network request
- deterministic structured artifacts generated locally
- useful for smoke tests and CI

### Fixture behavior

- no network request
- loads a stored JSON artifact from disk
- useful for prompt-regression and rendering tests

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
5. persist an `ai_artifacts` row with snapshot linkage and provider metadata
6. optionally write the raw JSON artifact to disk

## Current config surface

The AI configuration lives under `[ai]`.

Important fields:

- `enabled`
- `provider`
- `api_base_url`
- `api_key_env`
- `model`
- `reasoning_effort`
- `timeout_secs`
- `max_retries`
- `request_mode`
- `input_transport`
- `prompt_cache`
- `safety_identifier`

Safe defaults:

- disabled
- OpenAI provider selected but inactive
- stateless mode
- inline input
- prompt cache off
- no hidden uploads

## Persistence and schema

Current snapshot/AI persistence tables:

- `snapshot_exports`
- `snapshot_provenance_refs`
- `ai_artifacts`

These tables are used for:

- snapshot manifest persistence
- local evidence-to-record mapping
- saved AI review/compare artifacts
- prompt/schema/provider drift diagnosis

## Visual-system runtime

The TUI still uses the dedicated shared presentation layer under `src/ui/*`.

Important boundary:

- widgets never perform HTTP
- widgets never refresh tokens
- widgets never write to SQLite
- widgets never invoke the OpenAI provider directly
- the render path stays on persisted presentation models only

There is intentionally no freeform AI chat surface in this pass.

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
cargo run -- ai review /tmp/ringmaster-snapshot.json --dry-run
cargo run -- ai compare /tmp/ringmaster-snapshot.json /tmp/ringmaster-snapshot.json --dry-run
```

## Notes for future passes

- keep the AI boundary snapshot-first
- keep the TUI pure and read-only with respect to saved AI artifacts
- do not add freeform chat without a dedicated design pass
- do not enable tools, browsing, or direct database inspection in the provider path without a new privacy review
