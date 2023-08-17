use insta::glob;
use osm2rdf::parser::parse_block;
use osm2rdf::parser::Parser;
use osm2rdf::utils::Stats;
use osmnodecache::{CacheStore, HashMapCache};
use osmpbf::{BlobDecode, BlobReader};
use std::sync::Mutex;

#[test]
fn decode_osm_pbf_files() {
    glob!("../tests/fixtures", "**/*.osm.pbf", |file| {
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
    });
}
