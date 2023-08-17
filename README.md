# osm2rdf

[![GitHub](https://img.shields.io/badge/github-nyurik/osm2rdf-8da0cb?logo=github)](https://github.com/nyurik/osm2rdf)
[![crates.io version](https://img.shields.io/crates/v/osm2rdf.svg)](https://crates.io/crates/osm2rdf)
[![docs.rs docs](https://docs.rs/osm2rdf/badge.svg)](https://docs.rs/osm2rdf)
[![crates.io version](https://img.shields.io/crates/l/osm2rdf.svg)](https://github.com/nyurik/osm2rdf/blob/main/LICENSE-APACHE)
[![CI build](https://github.com/nyurik/osm2rdf/workflows/CI/badge.svg)](https://github.com/nyurik/osm2rdf/actions)

A tool to convert OpenStreetMap database dump into RDF TTL files for injesting into an RDF database

## Development
* This project is easier to develop with [just](https://github.com/casey/just#readme), a modern alternative to `make`. Install it with `cargo install just`.
* To get a list of available commands, run `just`.
* To run tests, use `just test`.
* On `git push`, it will run a few validations, including `cargo fmt`, `cargo clippy`, and `cargo test`.  Use `git push --no-verify` to skip these checks.
* Install `cargo install cargo-insta` to simplify running tests with [insta](https://insta.rs/docs/quickstart/).  Then run `cargo insta review` to review the changes, and `cargo insta test` to update the reference images.

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
  at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the
Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
