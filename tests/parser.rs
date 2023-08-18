use clap::Parser as _;
use flate2::read::GzDecoder;
use std::fs::File;
use std::io::Read;
use std::panic::catch_unwind;
use std::path::PathBuf;
use std::sync::Mutex;

use insta::glob;
use osm2rdf::parser::Parser;
use osm2rdf::utils::Stats;
use osm2rdf::{parser, Args};
use osmnodecache::{CacheStore, HashMapCache};
use osmpbf::{BlobDecode, BlobReader};
use temp_dir::TempDir;

#[test]
fn decode_osm_pbf_files() {
    glob!("../tests/fixtures", "**/*.os*.pbf", |file| {
        if let Err(_) = catch_unwind(|| {
            let reader = BlobReader::from_path(file).unwrap();
            let cache = HashMapCache::new();
            let stats = Mutex::new(Stats::default());
            let mut parser = Parser::new(&stats, cache.get_accessor(), 100);

            let mut result = Vec::new();
            for blob in reader {
                if let BlobDecode::OsmData(block) = blob.unwrap().decode().unwrap() {
                    parser.parse_block(block, |s| result.extend(s));
                };
            }
            insta::assert_debug_snapshot!(result);
        }) {
            panic!("Error while parsing file {}", file.display());
        }
    });
}

#[test]
fn generate_ttl() {
    let temp_dir = TempDir::new().unwrap();
    let temp_dir_path = temp_dir.path();

    let test_file = PathBuf::from(file!())
        .parent()
        .unwrap()
        .join("fixtures/osm2rdf/dense_test1.osm.pbf");

    // Parse a test file, generating output files in the temp directory
    parser::parse(Args::parse_from(&[
        "osm2rdf",
        "parse",
        test_file.to_str().unwrap(),
        temp_dir_path.to_str().unwrap(),
    ]))
    .unwrap();

    glob!(temp_dir_path, "*.ttl.gz", |file| {
        let file = File::open(file).unwrap();
        let mut reader = GzDecoder::new(&file);
        let mut ttl_file_content = String::new();
        reader.read_to_string(&mut ttl_file_content).unwrap();
        insta::assert_display_snapshot!(ttl_file_content);
    });
}
