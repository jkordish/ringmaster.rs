# Demo/Live Dashboard Parity Pass

## Goal

Make the dashboard feel like one product in demo and live mode by routing both through one canonical presenter contract, preserving panel skeletons across sparse states, and defaulting the dashboard to the most renderable recent day instead of blindly preferring the newest open day.

## Why

Demo currently lands on a richly populated closed-day fixture while live often defaults to sparse open-day coverage. The data difference is legitimate, but the presenter and default-day policy make the dashboard feel visually inconsistent and less trustworthy than it should.

## Current state

- Demo and live already share `build_state_from_snapshot` and `build_live_model`, but dashboard presentation logic is embedded directly in `src/app.rs`.
- Panel badges and sparse-state semantics are derived piecemeal from `TelemetryAvailability` plus per-panel booleans.
- Default day selection is `newest_day_index`, which prefers the most recent available day even if it cannot render the canonical dashboard well.
- Sparse panels fall back to generic scaffolds instead of a canonical presenter-driven identity contract.

## Desired state

- One explicit dashboard presenter layer computes canonical panel states for both demo and live.
- Dashboard panel states distinguish at least `Current`, `CurrentWithBaseline`, `BaselineOnly`, `HistoricalOnly`, `Empty`, `MissingScope`, and `Error`.
- Dashboard badges come from the presenter state rather than ad hoc renderer decisions.
- Default dashboard day selection prefers the latest complete/recent day with enough top-line daily coverage, while still allowing explicit navigation to open days.
- Snapshot and fixture coverage exercises full, baseline-only, historical-only, empty/missing-scope, and mixed-family dashboard states.

## Constraints

- Preserve the current shell, palette, and interaction model.
- No demo-only or live-only dashboard component variants.
- Keep panel identity stable across sparse states without replacing chart-led tiles with large placeholder prose.
- Do not revert unrelated in-progress work already in the tree.

## Risks

- Refactoring presenter logic inside `src/app.rs` could accidentally drift other screens if shared helpers are changed too broadly.
- Changing default-day selection can invalidate assumptions in existing tests and snapshots.
- Sparse skeleton tuning can subtly alter dashboard rendering density across viewports.

## File plan

- `src/app.rs`
- `src/components/dashboard.rs`
- `src/ui/telemetry.rs`
- `src/lib.rs`
- `docs/execplans/20260414-demo-live-dashboard-parity-pass.md`

## Milestones

- [ ] Introduce canonical dashboard presenter state and route all dashboard panels through it.
- [ ] Implement explicit default dashboard day selection that prefers the most renderable recent day.
- [ ] Add parity-focused tests and snapshot coverage for sparse/full dashboard states.

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo run -- doctor`

## Follow-up work

- If more screens need the same “best anchor day” semantics later, extract the selection policy into a reusable cross-screen anchor service instead of duplicating dashboard-specific heuristics.
