#!/usr/bin/env just --justfile

@_default:
    just --list --unsorted

# Clean all build artifacts
clean:
    cargo clean
    rm -f Cargo.lock

# Clean test snapshots
clean-snapshots:
    rm -rf tests/snapshots

# Run cargo fmt and cargo clippy
lint: fmt clippy

# Run cargo fmt
fmt:
    cargo +nightly fmt -- --config imports_granularity=Module,group_imports=StdExternalCrate

# Run cargo clippy
clippy:
    cargo clippy --workspace --all-targets --bins --tests --lib --benches -- -D warnings

# Build and open code documentation
docs:
    cargo doc --no-deps --open

# Run all tests
test:
    ./.cargo-husky/hooks/pre-push

# Run all tests, review, and approve them
bless: clean-snapshots
    cargo insta test --accept

# Run all tests, review, and approve them
review:
    cargo insta test --review
