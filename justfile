default:
  just --list

build:
  cargo build

release:
  cargo build --release

test:
  cargo test

lint:
  cargo clippy -- -D warnings

fmt:
  cargo fmt

fmt-check:
  cargo fmt --check

check: fmt-check lint test

test-unit:
  cargo test --lib

test-integration:
  cargo test --test '*'

run *args:
  cargo run -- {{args}}
