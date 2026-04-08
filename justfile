set shell := ["bash", "-cu"]

default:
    just check

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

test:
    cargo test --all

doctor:
    cargo run -- doctor

sync-fixture-smoke:
    cargo run -- sync once --dry-run --fixture-dir tests/fixtures/phase1

demo:
    cargo run -- demo

check: fmt-check clippy test doctor sync-fixture-smoke
