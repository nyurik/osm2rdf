use std::panic::catch_unwind;
use std::sync::Mutex;

use insta::glob;
use osm2rdf::parser::{parse_block, Parser};
use osm2rdf::utils::Stats;
use osmnodecache::{CacheStore, HashMapCache};
use osmpbf::{BlobDecode, BlobReader};
#[test]
fn decode_osm_pbf_files() {
    glob!("../tests/fixtures", "**/*.os*.pbf", |file| {
        if let Err(_) = catch_unwind(|| {
            let reader = BlobReader::from_path(file).unwrap();
            let cache = HashMapCache::new();
            let stats = Mutex::new(Stats::default());
            let mut parser = Parser::new(&stats, cache.get_accessor());

            let mut result = Vec::new();
            for blob in reader {
                if let BlobDecode::OsmData(block) = blob.unwrap().decode().unwrap() {
                    parse_block(block, &mut parser, |s| result.push(s));
                };
            }
            insta::assert_debug_snapshot!(result);
        }) {
            panic!("Error while parsing file {}", file.display());
        }
    });
}
