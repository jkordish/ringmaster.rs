# AI Workbench and First-Class TUI

## Goal

Make AI a first-class, privacy-explicit workflow in the TUI with a dedicated workbench, inline launch points, preflight confirmation, async run tracking, guided follow-up actions, and in-app browsing of snapshots, AI runs, and reports.

## Why

The earlier snapshot, AI artifact, report, and eval work made those capabilities durable, but the product still felt CLI-first for AI. This pass makes them visible, understandable, and useful where users already work inside the TUI.

## Current state

- The TUI now has a top-level `AI` workbench screen with launch, browser, trust, and detail regions.
- Existing screens route into the AI workbench through bounded inline launch points instead of direct provider calls.
- Every in-app AI launch routes through a typed preflight confirmation flow that shows the exact snapshot payload posture before confirmation.
- Snapshots, AI runs, and reports are now browsable through one in-app list/detail model with provenance and jump-back paths.
- AI runs persist lifecycle state locally in `ai_runs`, including queued, running, succeeded, failed, cancelled, and interrupted states.
- Review, compare, and follow-up artifacts render natively in the TUI rather than as raw JSON dumps.
- Deterministic UI and CLI smoke coverage now includes the AI screen and the major workbench states.

## Desired state

- The TUI has a top-level `AI` workbench screen.
- Existing screens expose carefully placed AI launch actions that route through the workbench.
- Every AI launch shows a compact preflight preview before any network activity.
- AI runs are orchestrated asynchronously and remain inspectable after completion or failure.
- Structured AI outputs render natively in the TUI with provenance and jump-back paths to local evidence.
- Snapshots, AI runs, and reports are browsable through one coherent in-app artifact model.

## Constraints

- Keep the app local-first and privacy-first.
- Keep widgets pure: no provider calls, DB writes, or token refresh in components.
- Preserve the `Event -> Action -> State -> Render` flow and avoid blocking the render path.
- Keep model interactions snapshot-bounded and stateless by default.
- No freeform chat box, hidden uploads, built-in tools, or live DB-to-model access in this pass.

## Risks

- Run orchestration could sprawl if lifecycle state is split across ad hoc UI-only models.
- Artifact browsing could become noisy if snapshots, runs, and reports do not share a coherent list/detail vocabulary.
- Guided follow-ups could drift into arbitrary prompting unless they are explicitly schema-bound and provenance-aware.
- The new AI surface can feel bolted on if inline entry points and workbench navigation do not reuse existing state and terminology.

## File plan

- `docs/execplans/20260410-ai-workbench-and-first-class-tui.md`
- `src/action.rs`
- `src/app.rs`
- `src/tui.rs`
- `src/lib.rs`
- `src/ai.rs`
- `src/ai_prompts.rs`
- `src/ai_prompts/follow_up_prompt_v1.md`
- `src/ai_prompts/follow_up_task_frame_v1.md`
- `src/store/migrations.rs`
- `src/store/queries.rs`
- `src/components/review.rs`
- `src/components/explain.rs`
- `src/components/patterns.rs`
- `src/components/dashboard.rs`
- `src/components/ops.rs`
- `src/components/ai.rs`
- `src/ui/snapshot.rs`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`
- `docs/IMPLEMENT.md`
- `docs/OPENAI_INTEGRATION.md`
- `docs/DESIGN_SYSTEM.md`

## Milestones

- [x] Milestone 1: add the exec plan, AI run registry + queries, typed preflight/request-preview models, and shared provenance formatting foundations.
- [x] Milestone 2: add the `AI` workbench screen, top-level navigation, artifact browser state, and Ops/doctor AI diagnostics.
- [x] Milestone 3: add preflight confirmation, async run orchestration, cancellation handling, and inline launch points from existing screens.
- [x] Milestone 4: add native TUI rendering for review/compare/follow-up artifacts, guided follow-up actions, and report/evidence navigation.
- [x] Milestone 5: extend deterministic UI coverage, update docs, and complete full verification.

## Verification

- Passed:
  - `cargo fmt --all --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test --all`
  - `cargo run -- doctor`
  - `cargo run -- snapshot export --demo --profile redacted --out /tmp/ringmaster-snapshot.json`
  - `cargo run -- ai review /tmp/ringmaster-snapshot.json --dry-run`
  - `cargo run -- ui snapshot --screen ai --demo --out-dir /tmp/ringmaster-ai-ui`
  - `cargo test --lib 'tui::tests::ai_workbench_smoke_path_covers_disabled_preflight_and_saved_run_detail' -- --exact`

## Notes from execution

- The main implementation was already partly in flight at the start of this run, so this pass focused on tightening the AI workbench integration, adding deterministic coverage, finishing the documentation sweep, and repairing the final compile/lint/test issues.
- The final repair set included:
  - a non-blocking report export path for TUI-triggered report generation
  - AI workbench render and lifecycle tests
  - CLI smoke coverage for `ui snapshot --screen ai`
  - schema-version test alignment after the `ai_runs` migration landed as schema version `15`
  - clippy cleanups and focused allowances on a few orchestration/view-model helpers whose shape is intentional

## Follow-up work

- Defer arbitrary chat, autonomous/background AI runs, built-in tools, and live DB-to-model access.
- Revisit richer compare-target suggestions only after the artifact browser and guided-action contracts prove stable.
- Consider deeper eval visualizations in the TUI only after the workbench’s operator and provenance surfaces settle.
