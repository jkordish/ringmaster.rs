set shell := ["bash", "-cu"]

default:
    just check

fmt:
    cargo fmt --all

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

test:
    cargo test --all

doctor:
    cargo run -- doctor

demo:
    cargo run -- demo

check: fmt clippy test
