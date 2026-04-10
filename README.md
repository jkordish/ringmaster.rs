# ringmaster.rs

`ringmaster.rs` is a local-first terminal app for exploring Oura data without handing your day-to-day health history to a hosted dashboard by default.

It gives you:

- a Ratatui interface for browsing recent signals, trends, context, patterns, reviews, and system status
- a SQLite-backed local cache with deterministic demo and fixture flows
- real Oura login and sync for supported families
- an optional snapshot-based OpenAI layer for bounded review, compare, report, and eval workflows

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
```

### Run the snapshot library and report workflow

```bash
cargo run -- snapshot export --demo --profile redacted --out /tmp/ringmaster-snapshot.json
cargo run -- snapshot list --demo
cargo run -- snapshot show /tmp/ringmaster-snapshot.json
cargo run -- ai review /tmp/ringmaster-snapshot.json --dry-run
cargo run -- ai compare /tmp/ringmaster-snapshot.json /tmp/ringmaster-snapshot.json --dry-run
cargo run -- ai runs list --demo
cargo run -- report export --from-snapshot /tmp/ringmaster-snapshot.json --format markdown --out /tmp/ringmaster-report.md
cargo run -- ai eval --fixture-dir tests/fixtures/ai
```

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
- a seven-screen TUI: Dashboard, Timeline, Trends, Explain, Patterns, Review, Status
- webhook-aware freshness and ops surfaces where Oura supports it
- deterministic demo, fixture, and smoke-test paths
- snapshot export plus a local snapshot catalog
- structured AI review/compare artifact persistence plus AI run browsing
- a Review-screen read-only AI artifact panel with local summary and provenance context
- Markdown and HTML report export from snapshots and AI runs
- a fixture-backed local eval flywheel for prompt/schema/model regressions

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
- Current execution plan for the snapshot library/report/eval pass:
  - [docs/execplans/20260410-phase8-snapshot-library-reports-and-eval-flywheel.md](docs/execplans/20260410-phase8-snapshot-library-reports-and-eval-flywheel.md)
- Current execution plan for the Review AI artifact panel follow-up:
  - [docs/execplans/20260410-phase9-review-ai-artifact-panel.md](docs/execplans/20260410-phase9-review-ai-artifact-panel.md)

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
cargo run -- report export --from-snapshot /tmp/ringmaster-snapshot.json --format markdown --out /tmp/ringmaster-report.md
cargo run -- ai eval --fixture-dir tests/fixtures/ai
```
