# Phase 7: Snapshot Export and OpenAI Briefing

## Goal

Add a privacy-aware, snapshot-first OpenAI analysis layer to `ringmaster.rs` with canonical snapshot exports, explicit redaction profiles, structured AI review/compare commands, local artifact persistence, and dry-run/fixture-backed validation.

## Why

The previous phases already established sync, persistence, explainability, patterns, reviews, and the visual system. This pass adds an optional external synthesis step without exposing the live database directly or turning the app into a freeform assistant.

## Current state

- The app already has fixture-backed sync seeding, derived review artifacts, deterministic CLI/TUI outputs, and a strongly typed SQLite store.
- There is no canonical snapshot export format.
- There is no provider abstraction for OpenAI or fixture-backed AI evaluation.
- There is no local persistence for AI review or compare artifacts.

## Desired state

- `snapshot export` produces a versioned JSON bundle that is useful on its own and is the only artifact ever sent to OpenAI.
- Privacy profiles control what leaves the machine, with `redacted` as the default.
- `ai review` and `ai compare` read local snapshots, request strict Structured Outputs from the Responses API, render human-readable briefings locally, and persist machine-safe artifacts with provenance.
- Dry-run and fixture-backed paths allow testing without live API calls.
- Docs clearly explain what is sent, what is not sent, and why there is no chat surface.

## Constraints

- Local-first, privacy-first, single-crate architecture.
- Ratatui remains pure and does not perform network I/O or writes from widgets.
- No direct database-to-model access, no hidden uploads, no tool-enabled AI calls, and no freeform chat.
- Use the OpenAI Responses API with Structured Outputs.
- Keep the repo compileable and validated after each milestone.

## Risks

- Snapshot schemas and Rust structs could drift if generated schemas are not validated in tests.
- Redaction bugs could leak identifiers or free-text notes in the default profile.
- Provider logic could spread across unrelated modules if the boundary is not enforced early.
- Prompt behavior could drift silently without golden fixture coverage.
- Optional TUI integration could bloat scope if added before CLI and persistence are stable.

## File plan

- `Cargo.toml`
- `src/cli.rs`
- `src/config.rs`
- `src/error.rs`
- `src/lib.rs`
- `src/app.rs`
- `src/store/db.rs`
- `src/store/migrations.rs`
- `src/store/queries.rs`
- `src/snapshot/*`
- `src/ai/*`
- `tests/*`
- `tests/fixtures/*`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`
- `docs/IMPLEMENT.md`
- `docs/OPENAI_INTEGRATION.md`

## Milestones

- [x] Milestone 1: add the exec plan, snapshot export models, privacy profiles, CLI surface, and persistence schema.
- [x] Milestone 2: implement deterministic snapshot export, manifest/provenance persistence, and redaction coverage.
- [x] Milestone 3: add the AI provider abstraction, OpenAI/fixture/dry-run providers, and `ai review`.
- [x] Milestone 4: add `ai compare`, persisted AI artifacts, rendered briefings, and schema/prompt regression coverage.
- [x] Milestone 5: finish docs and full validation. The optional read-only TUI brief viewer was intentionally deferred to keep this pass bounded and compileable.

## Implementation notes

- Snapshot exports landed in `src/snapshot.rs` as a single versioned `SnapshotBundleV1` with deterministic hashing, manifest persistence, and local provenance refs.
- The default redaction profile strips obvious identifiers and suppresses review-signal free text, while keeping derived metrics and follow-up targets useful.
- The provider seam landed in `src/ai.rs` with `dry_run`, `fixture`, and `openai` implementations.
- The real provider path uses the OpenAI Responses API with strict Structured Outputs and stateless defaults.
- `snapshot export`, `ai review`, and `ai compare` all landed in the CLI and persist their local artifacts through typed store queries instead of ad hoc file handling.
- The optional TUI brief viewer was assessed and intentionally deferred so this pass could stay focused on the snapshot boundary, provider seam, persistence, and docs.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- snapshot export --demo --profile redacted --out /tmp/ringmaster-snapshot.json`
- `cargo run -- ai review /tmp/ringmaster-snapshot.json --dry-run`
- `cargo run -- ai compare /tmp/ringmaster-snapshot.json /tmp/ringmaster-snapshot.json --dry-run`

## Verification status

- Completed on `2026-04-10`.
- The implementation was also checked incrementally with targeted `cargo test --lib ...` runs while fixing snapshot hash validation drift in the new export path.

## Follow-up work

- Richer AI browsing and drill-down in the TUI if the read-only brief viewer proves useful.
- Optional stateful provider mode and prompt caching beyond the conservative default.
- Broader snapshot scopes and richer comparison presets once the base schema stabilizes.
