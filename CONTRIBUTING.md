# Contributing

## Principles

- Keep changes small, typed, and verifiable.
- Update docs when behavior or architecture changes.
- Preserve local-first behavior and pure UI boundaries.
- Prefer real end-to-end slices over more scaffolding.

## Before opening a PR

Run:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo run -- doctor
```

If your change affects auth, sync, schema, or multi-file architecture, add or update an exec plan under `docs/execplans/`.

## Design boundaries

- `src/components/*` render state only
- `src/oura/*` owns auth, API, and sync logic
- `src/store/*` owns persistence and query boundaries
- `src/app.rs` shapes persisted/auth state into presentation models

## Documentation

Keep these files accurate in the same change when relevant:

- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/STATUS.md`
- `docs/IMPLEMENT.md`
- the active exec plan
