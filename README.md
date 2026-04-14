# ringmaster.rs

`ringmaster.rs` is a local-first terminal app for exploring Oura data without handing your day-to-day health history to a hosted dashboard by default.

It gives you:

- a Ratatui interface for browsing recent signals, trends, context, patterns, reviews, and system status
- a SQLite-backed local cache with deterministic demo and fixture flows
- real Oura login and sync for supported families
- an optional snapshot-based OpenAI layer for bounded review, compare, follow-up, report, and eval workflows that inherits the same evidence and safety rules as the deterministic product
- a first-class in-app AI workbench with explicit preflight, saved-run and eval browsing, and local evidence jump-backs

The design goal is simple: useful local insight first, optional external analysis second, and no surprise data sharing.

The crate is app-first. The `lib` target exists to support the binary, tests, and local tooling, and the package is not published as a general-purpose library crate.

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

On Linux, `auth login` stores tokens through the desktop Secret Service keyring by default. If token persistence fails, make sure a provider such as `gnome-keyring` or `KeePassXC` is running and unlocked. For headless boxes, you can explicitly opt into local file storage instead:

```bash
export RINGMASTER_OURA_SECRET_BACKEND=file
export RINGMASTER_OURA_SECRET_FILE="$HOME/.local/state/ringmaster/secrets/oura-tokens.json"
```

The file backend is opt-in only. Ringmaster will not silently fall back from secure storage to plaintext local files.

The default auth request now tracks Oura's broader current scope surface:

- `email`
- `personal`
- `daily`
- `heartrate`
- `tag`
- `workout`
- `session`
- `spo2`
- `ring_configuration`
- `stress`
- `heart_health`

Today the local product fully uses the baseline sync scopes plus the currently wired stress, heart-health, sleep-physiology, and `spo2` reads. `ring_configuration` and `email` are still surfaced in auth/doctor/status as future-ready capability slots rather than hidden or silently ignored.

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
cargo run -- ui snapshot --demo --screen dashboard --size compact --size medium --size wide --ansi-sidecar --color-mode truecolor --color-mode mono --out-dir /tmp/ringmaster-dashboard-ui
cargo run -- ui snapshot --screen explain --screen patterns --screen review --demo --out-dir /tmp/ringmaster-telemetry-ui
cargo run -- ui snapshot --screen ai --demo --out-dir /tmp/ringmaster-ai-ui
cargo run -- ui snapshot --screen status --demo --out-dir /tmp/ringmaster-status-ui
```

When `--ansi-sidecar` is enabled, Ringmaster writes the usual stable `.txt` artifacts plus color-aware `.ansi` sidecars for visual QA. For regression work, prefer explicit `truecolor` and `mono`; if you omit `--color-mode`, Ringmaster defaults to environment-driven `current` plus `mono`.

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

## Keyboard model

The TUI now follows one standard navigation grammar across Dashboard, Timeline, Trends, Explain, Patterns, Review, AI, and Status.

- `Tab` / `Shift+Tab` move between major regions such as `Views`, local controls, lists, and detail panes.
- Arrow keys move within the focused tabset, pager, chart, or list according to pane type.
- `Enter` / `Space` activate the focused control or commit the current selection.
- `Esc` closes help, closes search, dismisses a transient panel, or backs out one interaction layer.
- `Ctrl+F` opens search in the current searchable context.
- `?` opens a scoped keyboard-help overlay.

Search, help, and AI preflight now all behave as strict modal overlays: they trap interaction while open, restore the invoking region when closed, and keep background screen shortcuts inactive.

Pane behavior is now consistent by pane type:

- selectors and tabs use `Left` / `Right` plus `Home` / `End`
- lists use `Up` / `Down` plus `Home` / `End` and `PageUp` / `PageDown`
- detail panes use `Enter` / `Space` and `Esc` to return cleanly
- `Esc` backs out region-by-region instead of jumping straight to top-level navigation

Only panes with their own keyboard contract become major focus stops. Timeline now exposes visible window and overlay selectors, Explain and Patterns expose visible overlay-family selectors, and the AI workbench exposes a dedicated artifact-actions pane. Read-mostly screens such as Explain and Status still stay lean everywhere else instead of pretending that every visible subpanel is independently operable.

Wide layouts keep the primary `Views` navigation visible. Compact layouts keep the same interaction model while collapsing secondary content more aggressively. Optional expert aliases still exist, but they are supplemental rather than required. The canonical reference lives in [docs/KEYBINDINGS.md](docs/KEYBINDINGS.md).

## AI in the TUI

AI is now a top-level product workflow, not a CLI-only add-on.

- the dedicated `AI` workbench is one of the visible `Views` tabs
- the workbench follows the same region model as the rest of the app: `Views`, browser tabs, launch points, saved artifacts, artifact actions, and read-only artifact detail
- `Ctrl+F` searches saved-artifact lists and `?` opens the current keyboard help without leaving the screen
- every launch routes through an explicit preflight that shows snapshot scope, privacy profile, provider/model, stateless mode, tools-disabled status, content classes, payload size estimate, and the exact local artifact path that will be sent
- saved-artifact actions are now visible canonical controls instead of hidden letter-only workflows
- the workbench browses saved snapshots, AI runs, reports, and persisted eval runs in one place
- saved AI runs render structured findings, evidence, counterevidence, uncertainty, and provenance directly in the TUI
- saved eval runs render fixture manifest summaries, baseline-vs-candidate rollups, failing graders first, and lineage back to saved snapshots, AI runs, and reports when those local handles are available
- bounded follow-up actions such as expanding evidence, surfacing counterevidence, rerunning with another privacy profile/model, and generating a report are available without adding a freeform chat box
- the `Status` screen now surfaces latest eval health so regressions show up in the same operator surface as provider and sync readiness

The workbench is intentionally guided and snapshot-first. There is still no arbitrary chat prompt, no direct database-to-model path, and no hidden uploads.

## Scientific guardrails

Ringmaster now uses a typed three-tier evidence model across Review, Explain, Patterns, reports, snapshots, and AI outputs.

- `guideline_backed` claims can use stable general-adult public-health anchors where the registry explicitly allows them
- `evidence_informed` claims stay cautious, contextual, and limitation-aware
- `exploratory` claims are visibly marked as exploratory, trend-only, or context-only
- sensitive domains such as `SpO₂` and consumer sleep-tech outputs carry explicit caution rails and are not rendered as diagnostic or screening tools
- one active local population profile is configured explicitly, never inferred silently
- registry-backed guidance now resolves as `population-specific`, `general-adult-only` fallback, or `unavailable`
- sensitive metrics such as `SpO₂`, `HRV`, readiness/stress/resilience composites, and cardiovascular-age-style metrics do not silently inherit stronger language for unsupported populations
- Review cards now prioritize sensitive caution badges, and Review detail panes surface population fallback or unavailable scope directly instead of hiding it in prose
- Status and `cargo run -- doctor` both expose the evidence-registry version plus stale-review health so scientific maintenance shows up as an operational concern
- the product remains non-diagnostic: no diagnosis, no treatment recommendations, and no disease-screening positioning

The full contract lives in [docs/EVIDENCE_MODEL.md](docs/EVIDENCE_MODEL.md), and the maintenance workflow lives in [docs/EVIDENCE_MAINTENANCE.md](docs/EVIDENCE_MAINTENANCE.md).

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

Population-aware guidance now has an explicit local config surface:

```toml
[guidance]
active_population_profile = "general_adult"
```

Supported values in this phase are `general_adult`, `older_adult`, `pregnancy_postpartum`, `shift_worker`, and `athlete_high_training_load`. The app will show when a claim is population-specific, when it is falling back to general-adult guidance, and when no interpretation is available for the active profile.

For the complete config and runtime behavior, use the docs below instead of the README.

## Docs map

- Product/runtime guide: [docs/IMPLEMENT.md](docs/IMPLEMENT.md)
- Architecture and boundaries: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Current shipped status: [docs/STATUS.md](docs/STATUS.md)
- Navigation and keyboard model: [docs/KEYBINDINGS.md](docs/KEYBINDINGS.md)
- HCI research and audit for this pass:
  - [docs/HCI_NAVIGATION_RESEARCH.md](docs/HCI_NAVIGATION_RESEARCH.md)
  - [docs/NAVIGATION_AUDIT.md](docs/NAVIGATION_AUDIT.md)
- Evidence model and scientific claims policy: [docs/EVIDENCE_MODEL.md](docs/EVIDENCE_MODEL.md)
- Evidence maintenance workflow: [docs/EVIDENCE_MAINTENANCE.md](docs/EVIDENCE_MAINTENANCE.md)
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
cargo run -- ui snapshot --demo --out-dir /tmp/ringmaster-nav-ui
```

For the navigation model specifically, the deterministic smoke path now lives in the test suite and exercises top-level screen switching, region traversal, search open/close, help open/close, and detail back-out behavior through the same reducer and binding paths used by the live TUI.
