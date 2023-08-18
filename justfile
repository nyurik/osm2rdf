#!/usr/bin/env just --justfile

@_default:
    just --list --unsorted

# Clean all build artifacts
clean:
    cargo clean
    rm -f Cargo.lock

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
bless:
    cargo insta test --accept --unreferenced=auto

_osm-to-pbf DIR FORMAT="pbf" PREFIX="":
    #!/usr/bin/env bash
    set -euo pipefail
    shopt -s nullglob
    for ext in "osm" "osh"; do
        for file in tests/fixtures/{{ DIR }}/src/*.${ext}; do
            echo "Converting ${file} to {{ FORMAT }}..."
            osmium cat --no-progress --overwrite --input-format ${ext} --output-format ${ext}.{{ FORMAT }} -o "tests/fixtures/{{ DIR }}/{{ PREFIX }}$(basename "$file").pbf" "${file}"
        done
    done

# Regenerate PBF files from OSM source files in tests/fixtures
gen-pbf: (_osm-to-pbf "libosmium") (_osm-to-pbf "osm2rdf" "pbf,pbf_dense_nodes=true" "dense_") (_osm-to-pbf "osm2rdf" "pbf,pbf_dense_nodes=false" "nodense_")

# Run all tests, review, and approve them
review:
    cargo insta test --review --unreferenced=auto
