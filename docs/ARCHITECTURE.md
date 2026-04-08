# ARCHITECTURE.md

## Current state

This file describes the intended architecture direction for `ringmaster.rs`.

The repository currently contains a compileable placeholder shell. The first serious implementation pass should keep the shape of the boundaries below even as it replaces the placeholder internals.

## High-level design

```text
CLI / TUI input
      ↓
    App state + actions
      ↓
  pure component rendering
      ↓
     Ratatui

sync requests
      ↓
 Oura auth/client/sync
      ↓
     store layer
      ↓
 typed view data for UI
```

## Boundaries

### UI
- `src/action.rs`
- `src/app.rs`
- `src/tui.rs`
- `src/components/*`

Responsibilities:
- app state
- event loop
- screen navigation
- rendering

Must not do:
- token refresh
- HTTP requests
- DB writes

### Oura integration
- `src/oura/auth.rs`
- `src/oura/client.rs`
- `src/oura/models.rs`
- `src/oura/sync.rs`

Responsibilities:
- OAuth flow
- typed API client
- sync orchestration
- capability handling

### Storage
- `src/store/db.rs`
- `src/store/migrations.rs`
- `src/store/queries.rs`

Responsibilities:
- database path resolution
- migrations
- typed query boundaries
- source-of-truth persistence for rendered data

## Bootstrap implementation guidance

Phase 0 / 1 should focus on:
- real CLI
- deterministic demo mode
- Ratatui shell
- diagnostics
- migrations
- typed store and Oura seams

Do not block the first useful version on webhook infrastructure.
