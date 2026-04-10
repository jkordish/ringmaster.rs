# STATUS.md

## Purpose

This file is the current truth for the repository during the `snapshot-export-and-openai-briefing` pass completed on `2026-04-10`. It records what landed, what was verified, and what remains intentionally deferred.

## Baseline before this pass

Verified before implementation:

- the local-first CLI, sync, derive, review, webhook, and visual-system flows were already working
- `cargo fmt --all --check` passed
- `cargo clippy --all-targets --all-features -- -D warnings` passed
- `cargo test --all` passed
- `cargo run -- doctor` passed

Primary gaps before this pass:

- there was no canonical snapshot export artifact
- there was no privacy-profiled export path
- there was no optional OpenAI provider seam
- there was no persisted local store for AI review or compare artifacts
- there was no bounded machine-safe AI contract on top of the existing review and pattern layers

## Current implemented truth

The repository now includes:

- a canonical `snapshot export` command that produces a versioned JSON snapshot bundle
- explicit privacy profiles:
  - `redacted` as the default
  - `balanced`
  - `full`
- deterministic snapshot serialization plus stable snapshot hashing
- snapshot manifest persistence in SQLite
- opaque local provenance references so exported evidence can map back to local records without leaking identifiers
- bounded `ai review <snapshot-path>` and `ai compare <snapshot-a> <snapshot-b>` commands
- a dedicated AI provider seam with:
  - `dry_run`
  - fixture-backed replay
  - optional OpenAI Responses API execution
- strict Structured Outputs contracts for review and compare artifacts instead of prose parsing
- locally rendered human-readable briefings derived from structured JSON artifacts
- local persistence for AI outputs, including:
  - snapshot linkage
  - provider/model metadata
  - prompt/schema versions
  - run mode
  - privacy profile
  - created-at timestamps
- conservative OpenAI defaults:
  - disabled unless explicitly enabled
  - stateless requests by default
  - no tools enabled
  - no web search, file search, or remote retrieval
- fixture-backed and dry-run-friendly CLI tests for export, review, and compare

## Snapshot/export capabilities that now work

- `ringmaster snapshot export --demo --profile redacted --out /tmp/ringmaster-snapshot.json`
- bounded scopes:
  - `today`
  - `week`
  - `day:YYYY-MM-DD`
  - `range:YYYY-MM-DD..YYYY-MM-DD`
- deterministic JSON output in:
  - pretty mode
  - compact mode
- export metadata that records:
  - app version
  - schema version
  - generated timestamp
  - scope
  - privacy profile
  - source mode
  - snapshot hash
- derived snapshot content that can be useful without AI:
  - freshness and trust metadata
  - capability coverage
  - record counts
  - selected metrics and baselines
  - trend summaries
  - context events
  - pattern summaries
  - review signals
  - local follow-up targets

## OpenAI analysis capabilities that now work

- `ringmaster ai review <snapshot-path>`
- `ringmaster ai compare <snapshot-a> <snapshot-b>`
- dry-run mode for both commands without any API call
- fixture-backed review and compare runs for regression testing
- persisted local AI artifacts for both review and compare
- structured outputs that include:
  - overview
  - findings
  - limitations
  - evidence references
  - uncertainty markers
  - local follow-up targets

## Privacy and safety truth

The current behavior is intentionally conservative:

- the OpenAI layer is opt-in
- no user data is uploaded unless the user explicitly runs an AI command against a local snapshot
- the exported snapshot is the only artifact the provider can inspect
- `redacted` removes obvious personal/account identifiers and omits free-text review-signal payloads by default
- provider config lives separately from auth/sync config
- API keys are read from an env var, not stored in SQLite artifacts
- logs never need the API key or snapshot payload to explain success or failure

## Versioning truth

The pass now persists and documents:

- snapshot schema version: `ringmaster.snapshot.v1`
- review output schema version: `ringmaster.ai.review.v1`
- compare output schema version: `ringmaster.ai.compare.v1`
- prompt versions:
  - `review_prompt_v1`
  - `compare_prompt_v1`

## Tests now in place

Coverage now includes:

- snapshot scope resolution
- redacted export leakage checks
- snapshot export manifest + provenance persistence
- AI artifact persistence
- CLI parsing for snapshot and AI command families
- dry-run review rendering and persistence
- fixture-backed review rendering
- dry-run compare rendering
- provider-disabled failure behavior
- schema-generation sanity checks for review and compare outputs
- full-library regression coverage with the new phase-7 surfaces enabled

## Verification completed for this pass

Verified on `2026-04-10` after implementation:

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- snapshot export --demo --profile redacted --out /tmp/ringmaster-snapshot.json`
- `cargo run -- ai review /tmp/ringmaster-snapshot.json --dry-run`
- `cargo run -- ai compare /tmp/ringmaster-snapshot.json /tmp/ringmaster-snapshot.json --dry-run`

## Intentionally deferred

- freeform chat or arbitrary natural-language Q&A over the live database
- any direct database-to-OpenAI pipeline
- tool-enabled or browsing-enabled OpenAI runs
- a new AI chat screen in the TUI
- file-upload transport for OpenAI requests
- richer saved-brief browsing in the TUI beyond the existing CLI and persistence layer
- hosted relay services, notifications, packaging, installers, and release automation
