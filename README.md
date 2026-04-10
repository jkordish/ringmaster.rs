# ringmaster.rs

`ringmaster.rs` is a local-first terminal app for exploring Oura data without handing your day-to-day health history to a hosted dashboard by default.

It gives you:

- a Ratatui interface for browsing recent signals, trends, context, patterns, reviews, and system status
- a SQLite-backed local cache with deterministic demo and fixture flows
- real Oura login and sync for supported families
- an optional snapshot-based OpenAI layer for bounded review, compare, follow-up, report, and eval workflows
- a first-class in-app AI workbench with explicit preflight, saved-run and eval browsing, and local evidence jump-backs

The design goal is simple: useful local insight first, optional external analysis second, and no surprise data sharing.

## Start here

If you just want to see the product:

```bash
cargo run -- tui --demo
```

If you want to check your local setup:

```bash
cargo run -- doctor
```

If you want to connect your own Oura account:

```bash
cargo run -- auth login
cargo run -- sync once
cargo run -- tui
```

## Common workflows

### Explore locally

```bash
cargo run -- tui
cargo run -- tui --demo
cargo run -- review today --demo
cargo run -- review week --demo
```

### Validate the UI in CI or locally

```bash
cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-ui-snapshots
cargo run -- ui snapshot --screen ai --demo --out-dir /tmp/ringmaster-ai-ui
cargo run -- ui snapshot --screen status --demo --out-dir /tmp/ringmaster-status-ui
```

### Run the snapshot library and report workflow

```bash
cargo run -- snapshot export --demo --profile redacted --out /tmp/ringmaster-snapshot.json
cargo run -- snapshot list --demo
cargo run -- snapshot show /tmp/ringmaster-snapshot.json
cargo run -- ai review /tmp/ringmaster-snapshot.json --dry-run
cargo run -- ai compare /tmp/ringmaster-snapshot.json /tmp/ringmaster-snapshot.json --dry-run
cargo run -- ai runs list --demo
cargo run -- ui snapshot --screen ai --demo --out-dir /tmp/ringmaster-ai-ui
cargo run -- report export --from-snapshot /tmp/ringmaster-snapshot.json --format markdown --out /tmp/ringmaster-report.md
cargo run -- ai eval --fixture-dir tests/fixtures/ai
```

## AI in the TUI

AI is now a top-level product workflow, not a CLI-only add-on.

- `7` opens the dedicated `AI` workbench screen.
- `a` and `c` launch snapshot-bounded review or compare work from `Dashboard`, `Explain`, `Patterns`, `Review`, and the workbench itself.
- every launch routes through an explicit preflight that shows snapshot scope, privacy profile, provider/model, stateless mode, tools-disabled status, content classes, payload size estimate, and the exact local artifact path that will be sent
- the workbench browses saved snapshots, AI runs, reports, and persisted eval runs in one place
- saved AI runs render structured findings, evidence, counterevidence, uncertainty, and provenance directly in the TUI
- saved eval runs render fixture manifest summaries, baseline-vs-candidate rollups, failing graders first, and lineage back to saved snapshots, AI runs, and reports when those local handles are available
- bounded follow-up actions such as expanding evidence, surfacing counterevidence, rerunning with another privacy profile/model, and generating a report are available without adding a freeform chat box
- the `Status` screen now surfaces latest eval health so regressions show up in the same operator surface as provider and sync readiness

The workbench is intentionally guided and snapshot-first. There is still no arbitrary chat prompt, no direct database-to-model path, and no hidden uploads.

## Privacy defaults

The optional OpenAI integration is intentionally narrow.

- The model never reads the live database directly.
- The only outbound artifact is a snapshot you export yourself.
- `redacted` is the default snapshot profile.
- API requests are stateless by default.
- No freeform chat, prompt textbox, browsing, or tool-enabled agent behavior is included in this pass.

If you want the full contract, read [docs/OPENAI_INTEGRATION.md](docs/OPENAI_INTEGRATION.md).

## What works today

- loopback OAuth login and local token/session persistence
- local sync and derived review/pattern/context layers
- an eight-screen TUI: Dashboard, Timeline, Trends, Explain, Patterns, Review, AI, Status
- webhook-aware freshness and ops surfaces where Oura supports it
- deterministic demo, fixture, and smoke-test paths
- snapshot export plus a local snapshot catalog
- a dedicated AI workbench with inline launch points, explicit preflight, async run tracking, and in-app artifact browsing
- structured AI review/compare/follow-up artifact persistence plus AI run browsing
- local jump-backs from AI findings to Review / Explain / Patterns / Timeline evidence when the saved export refs are resolvable
- Markdown and HTML report export from snapshots and AI runs
- a fixture-backed local eval flywheel for prompt/schema/model regressions, including an in-app eval browser and Status eval health

## Minimal config

`ringmaster.rs` uses XDG-style paths by default:

- config: `~/.config/ringmaster/config.toml`
- state: `~/.local/state/ringmaster`
- cache: `~/.cache/ringmaster`

Secrets stay in environment variables, not plaintext config:

```bash
export RINGMASTER_OURA_CLIENT_SECRET="your-oura-client-secret"
export RINGMASTER_WEBHOOK_VERIFICATION_TOKEN="your-webhook-verification-token"
```

For the complete config and runtime behavior, use the docs below instead of the README.

## Docs map

- Product/runtime guide: [docs/IMPLEMENT.md](docs/IMPLEMENT.md)
- Architecture and boundaries: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Current shipped status: [docs/STATUS.md](docs/STATUS.md)
- OpenAI snapshot flow and privacy model: [docs/OPENAI_INTEGRATION.md](docs/OPENAI_INTEGRATION.md)
- Eval workflow and grading rules: [docs/EVALS.md](docs/EVALS.md)
- Visual system references:
  - [docs/DESIGN_AUDIT.md](docs/DESIGN_AUDIT.md)
  - [docs/DESIGN_SYSTEM.md](docs/DESIGN_SYSTEM.md)
- Snapshot library, reports, and eval workflow plan:
  - [Snapshot library, report export, and eval flywheel](docs/execplans/20260410-snapshot-library-reports-and-eval-flywheel.md)
- AI workbench plan:
  - [AI workbench and first-class TUI](docs/execplans/20260410-ai-workbench-and-first-class-tui.md)
- Eval lab and regression console plan:
  - [AI eval lab and regression console](docs/execplans/20260410-ai-eval-lab-and-regression-console.md)

## Development notes

- Rust baseline: `rust-version = 1.88`
- canonical verification:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo run -- doctor
cargo run -- snapshot export --demo --profile redacted --out /tmp/ringmaster-snapshot.json
cargo run -- snapshot list --demo
cargo run -- ai review /tmp/ringmaster-snapshot.json --dry-run
cargo run -- ai runs list --demo
cargo run -- ui snapshot --screen ai --demo --out-dir /tmp/ringmaster-ai-ui
cargo run -- ui snapshot --screen status --demo --out-dir /tmp/ringmaster-status-ui
cargo run -- report export --from-snapshot /tmp/ringmaster-snapshot.json --format markdown --out /tmp/ringmaster-report.md
cargo run -- ai eval --fixture-dir tests/fixtures/ai
```
