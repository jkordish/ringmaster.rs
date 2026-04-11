# Snapshot Library, Report Export, and Eval Flywheel

## Goal

Turn the earlier snapshot/OpenAI integration into a durable local workflow with a snapshot catalog, AI run registry, report export, and a local eval flywheel.

## Why

The earlier snapshot/OpenAI work proved the snapshot boundary and provider seam. This pass makes those artifacts useful over time by making them browsable, exportable, comparable, and safer to evolve.

## Current state

- `snapshot export` persists canonical snapshot artifacts and provenance references.
- `snapshot list` and `snapshot show` browse the local snapshot catalog.
- `ai review` and `ai compare` persist AI artifacts with payload JSON, summary caches, and request fingerprints.
- `ai runs list` and `ai runs show` browse the local AI run registry.
- `report export` renders Markdown and HTML reports from snapshots or AI runs and persists report manifests.
- prompts and task framing now live in versioned files under `src/ai_prompts/*`.
- `ai eval` runs deterministic fixture-backed grading and persists eval summaries.

## Desired state

- `snapshot list` and `snapshot show` make saved snapshot artifacts discoverable and inspectable.
- `ai runs list` and `ai runs show` make persisted AI artifacts discoverable and inspectable.
- `report export` builds Markdown and HTML reports from snapshots and AI runs and persists report manifests with provenance.
- `ai eval` runs deterministic local fixture-based evaluations with persisted summary history.
- Prompt/schema/template versioning is explicit, centralized, and attached to persisted artifacts.
- Request construction is canonical per AI task family and friendly to future prompt caching without changing product semantics.

## Constraints

- Local-first, privacy-first, single-crate architecture.
- Ratatui remains pure and this pass stays CLI-first; no new artifact browser screen lands in the TUI.
- No direct database-to-model access, hidden uploads, tool-enabled AI calls, or freeform chat.
- `redacted` remains the default privacy profile.
- Requests stay stateless by default and OpenAI usage remains explicit and opt-in.
- Keep the repo compileable after each milestone and update docs continuously.

## Risks

- Catalog metadata could accidentally leak sensitive fields if summaries are derived from the wrong snapshot data.
- Prompt/schema/template drift could become harder to track if versioning is only partially centralized.
- Report rendering could fork semantically between Markdown and HTML if the shared document model is not established first.
- Eval persistence could become noisy or expensive if it stores too much detail by default.
- Migration scope could grow if report/eval registries are not kept lightweight.

## File plan

- `docs/execplans/20260410-snapshot-library-reports-and-eval-flywheel.md`
- `src/cli.rs`
- `src/lib.rs`
- `src/ai.rs`
- `src/config.rs`
- `src/store/db.rs`
- `src/store/migrations.rs`
- `src/store/queries.rs`
- `src/snapshot.rs`
- `src/report/*`
- `src/eval/*`
- `src/ai_prompts/*`
- `tests/smoke_cli.rs`
- `tests/fixtures/ai/*`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`
- `docs/IMPLEMENT.md`
- `docs/OPENAI_INTEGRATION.md`
- `docs/EVALS.md`

## Milestones

- [x] Milestone 1: add the exec plan, storage schema changes, typed registry models, and CLI routing for the new command families.
- [x] Milestone 2: implement the snapshot catalog and AI run registry with deterministic list/show behavior and lineage-safe metadata.
- [x] Milestone 3: implement `report export` with shared report view models, Markdown/HTML rendering, and persisted report manifests.
- [x] Milestone 4: centralize prompt/schema/template versioning and refactor canonical request builders with better dry-run inspection.
- [x] Milestone 5: implement `ai eval`, fixture datasets, graders, and persisted eval summary history.
- [x] Milestone 6: finish docs, privacy/provenance hardening, validation, and close out completed versus deferred work.

## Verification

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

## Follow-up work

- Add a dedicated read-only TUI artifact browser once the library semantics and CLI workflows settle.
- Consider optional explicit delete/prune commands for local artifact cleanup if the catalog grows.
- Explore batch/archive processing on top of the registry abstractions without making it the default user-facing flow.
- Evaluate prompt caching hints beyond the conservative default once request builders stabilize.
