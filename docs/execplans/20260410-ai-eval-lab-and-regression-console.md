# AI Eval Lab and Regression Console

## Goal

Make evals a first-class in-app workflow by extending the existing AI workbench with an `Evals` browser/detail tab, richer persisted eval detail, native failing-grader rendering, lineage back to saved local artifacts, and Status eval health.

## Why

The earlier snapshot/report/eval work made evals durable and deterministic, and the AI workbench work made the TUI artifact browser first-class, but evals were still effectively CLI-only. The app already loads `ai_eval_runs` into `LiveSnapshot`; this pass turns that persisted history into a usable operator surface without changing the snapshot-first, stateless-by-default AI contract.

## Current state

- `ringmaster ai eval --fixture-dir ...` loads a manifest, grades local candidate and baseline artifacts, prints a stdout summary, optionally exports JSON, and persists one summary row in `ai_eval_runs`.
- `LiveSnapshot` already carries `ai_eval_runs`, but the `AI` workbench only browses `Runs`, `Snapshots`, and `Reports`.
- `ai_eval_runs` currently stores summary scores and regression text only; it does not persist manifest, case, grader, or lineage detail needed for in-app inspection.
- Status only surfaces eval counts, not eval health.

## Desired state

- The AI workbench gets a fourth browser tab, `Evals`, rather than adding a new top-level screen.
- Selecting an eval run shows fixture manifest summary, candidate-vs-baseline rollup, per-case results, failing graders first, and linked local snapshots/runs/reports when lineage is available.
- Status exposes latest eval health and warns when the newest eval regressed or has failed cases.
- Demo and deterministic UI coverage include the eval tab and eval-health Status surfaces.

## Constraints

- Keep the app local-first and privacy-first.
- Keep widgets pure: no eval execution, DB writes, or provider work inside components.
- Keep eval browsing read-only; no live API calls, no chat, no built-in tools, and no DB-to-model path.
- Keep eval execution snapshot-first and stateless-by-default.

## Risks

- Recomputing eval detail from fixture files at render time would make historical browsing non-durable.
- Adding separate eval tables could increase schema complexity more than this pass needs.
- Linkage could become misleading if the UI guesses artifact matches instead of using explicit lineage handles.

## File plan

- `docs/execplans/20260410-ai-eval-lab-and-regression-console.md`
- `src/eval.rs`
- `src/store/migrations.rs`
- `src/store/queries.rs`
- `src/app.rs`
- `src/action.rs`
- `src/tui.rs`
- `src/components/ai.rs`
- `src/components/ops.rs`
- `src/ui/snapshot.rs`
- `tests/fixtures/ai/manifest.json`
- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`
- `docs/IMPLEMENT.md`
- `docs/EVALS.md`

## Milestones

- [x] Milestone 1: add the execplan, extend eval persistence with `details_json`, and upgrade the eval runtime to persist manifest/case/grader detail plus optional lineage.
- [x] Milestone 2: add the `Evals` browser tab, detail rendering, and linked-artifact jump behavior inside the AI workbench.
- [x] Milestone 3: extend Status eval health, seed deterministic demo eval data, and add render/snapshot coverage for eval and Status flows.
- [x] Milestone 4: update docs and run full verification.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`
- `cargo run -- ai eval --fixture-dir tests/fixtures/ai`
- `cargo run -- ui snapshot --screen ai --demo --out-dir /tmp/ringmaster-ai-ui`
- `cargo run -- ui snapshot --screen ops --demo --out-dir /tmp/ringmaster-ops-ui`

## Follow-up work

- If eval history grows enough to make `details_json` unwieldy, split case/grader detail into dedicated tables in a later pass.
- Consider interactive per-case cursoring only after the first read-only eval lab proves useful.
