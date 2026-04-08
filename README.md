# ringmaster.rs

A local-first Rust terminal app for exploring Oura Cloud data with a Ratatui-based interface.

This repository is intentionally seeded with strong docs and a compileable skeleton first. The goal is to let Codex or a human implement against a clean target instead of improvising the whole project from vapor.

## Current status

Bootstrap seed:

- guidance docs in place
- governance in place
- architecture direction defined
- compileable placeholder code in place
- ready for a Codex bootstrap pass

## Intended command surface

- `ringmaster tui`
- `ringmaster doctor`
- `ringmaster auth login`
- `ringmaster sync once`
- `ringmaster demo`

The current code only stubs these commands. The next step is to use the Codex prompt in `docs/CODEX_START_PROMPT.md` to turn the skeleton into a real app shell.

## Why this repo starts with docs

Codex reads `AGENTS.md` before doing work, and it performs better when it can verify changes and follow explicit project rules. This repo is set up to make that first long Codex run far less chaotic.

## Local development

### Commands

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo run -- doctor
cargo run -- demo
```

## Repo shape

```text
src/
  action.rs
  app.rs
  cli.rs
  config.rs
  error.rs
  tui.rs
  components/
  oura/
  store/
docs/
  CODEX_START_PROMPT.md
  EXECPLAN.md
  execplans/
```

## Next move

1. commit these seed files
2. open Codex in the repo
3. paste `docs/CODEX_START_PROMPT.md`
4. let it build the real phase-0 / phase-1 foundation
