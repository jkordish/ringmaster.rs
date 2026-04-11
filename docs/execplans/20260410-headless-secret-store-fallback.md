# Headless Secret Store Fallback

## Goal

Add an explicit opt-in file secret backend for Oura OAuth tokens so `ringmaster auth login` can work on headless Linux systems without a running Secret Service provider.

## Why

The current Linux path fails closed when `org.freedesktop.secrets` is unavailable, which is correct by default but leaves server and headless environments without a workable auth persistence path.

## Current state

- `keyring` is the only runtime secret backend.
- Linux requires a running Secret Service provider such as `gnome-keyring` or `KeePassXC`.
- `doctor` and `auth login` report backend failures clearly, but there is no supported fallback.

## Desired state

- `keyring` remains the default backend.
- Users can explicitly opt into `file` storage with config or env.
- The file backend stores the same token payload locally under the state dir by default, with private permissions.
- `doctor`, `auth login`, and docs make the selected backend clear.

## Constraints

- Keep the default behavior privacy-first and fail closed on missing secure storage.
- Do not silently fall back from `keyring` to plaintext file storage.
- Keep UI rendering pure; the change stays inside config/auth/doc/test surfaces.

## Risks

- File permission handling can be too loose on Unix if we are not careful.
- A backend selector added to config touches many tests and fixture constructors.
- Headless guidance can become confusing if the docs do not explain the opt-in clearly.

## File plan

- `docs/execplans/20260410-headless-secret-store-fallback.md`
- `src/config.rs`
- `src/oura/auth.rs`
- `src/lib.rs`
- `src/tui.rs`
- `src/ui/snapshot.rs`
- `src/webhook/receiver.rs`
- `README.md`
- `docs/ARCHITECTURE.md`

## Milestones

- [x] add backend selection to config with an explicit file backend path
- [x] implement the file secret store and integrate it with auth inspection/login/refresh
- [x] update tests and docs, then rerun full verification

## Verification

- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all`
- `cargo build`
- `cargo run -- doctor`

## Follow-up work

- Consider an explicit `auth logout` path that deletes file-backed tokens as well as keyring entries.
