# SKELETON.md

## Seed repo structure

```text
.
├── AGENTS.md
├── GOVERNANCE.md
├── README.md
├── SPEC.md
├── Cargo.toml
├── clippy.toml
├── justfile
├── rust-toolchain.toml
├── docs
│   ├── CODEX_START_PROMPT.md
│   ├── EXECPLAN.md
│   └── execplans
├── src
│   ├── action.rs
│   ├── app.rs
│   ├── cli.rs
│   ├── config.rs
│   ├── error.rs
│   ├── lib.rs
│   ├── main.rs
│   ├── tui.rs
│   ├── components
│   │   ├── dashboard.rs
│   │   ├── mod.rs
│   │   ├── ops.rs
│   │   ├── timeline.rs
│   │   └── trends.rs
│   ├── oura
│   │   ├── auth.rs
│   │   ├── client.rs
│   │   ├── mod.rs
│   │   ├── models.rs
│   │   └── sync.rs
│   └── store
│       ├── db.rs
│       ├── migrations.rs
│       ├── mod.rs
│       └── queries.rs
└── tests
    └── smoke_cli.rs
```

## Expected shape after the first serious Codex pass

- real CLI with `clap`
- real TUI event loop using `ratatui + crossterm + tokio`
- demo data provider
- logging, config, and doctor
- SQLite migrations and typed store seams
- Oura auth/client/sync scaffolding
- CI and docs aligned with implemented behavior
