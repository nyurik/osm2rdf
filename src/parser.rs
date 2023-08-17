use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::thread::{Builder, JoinHandle};

use bytesize::ByteSize;
use flate2::write::GzEncoder;
use flate2::Compression;
use geos::{CoordSeq, Geom, Geometry};
use osmnodecache::{Cache, CacheStore, DenseFileCache, DenseFileCacheOpts, HashMapCache};
use osmpbf::{
    BlobDecode, BlobReader, DenseNode, Info, Node, PrimitiveBlock, RelMemberType, Relation, Way,
};
use path_absolutize::Absolutize;
use rayon::iter::{ParallelBridge, ParallelIterator};
use regex::Regex;

use crate::str_builder::StringBuf;
use crate::utils::{to_utc, Element, Statement, Stats};
use crate::{Args, Command};
use lazy_static::lazy_static;

lazy_static! {
    /// Total length of the maximum "valid" local name is 60 (58 + first + last char)
    /// Local name may contain letters, numbers anywhere, and -:_ symbols anywhere except first and last position
    pub static ref RE_SIMPLE_LOCAL_NAME: Regex = Regex::new(r"^[0-9a-zA-Z_]([-:0-9a-zA-Z_]{0,58}[0-9a-zA-Z_])?$").unwrap();
    pub static ref RE_WIKIDATA_KEY: Regex = Regex::new(r"(.:)?wikidata$").unwrap();
    pub static ref RE_WIKIDATA_VALUE: Regex = Regex::new(r"^Q[1-9][0-9]{0,18}$").unwrap();
    pub static ref RE_WIKIDATA_MULTI_VALUE: Regex = Regex::new(r"^Q[1-9][0-9]{0,18}(;Q[1-9][0-9]{0,18})+$").unwrap();
    pub static ref RE_WIKIPEDIA_VALUE: Regex = Regex::new(r"^([-a-z]+):(.+)$").unwrap();
}

pub struct Parser<'a> {
    parent_stats: &'a Mutex<Stats>,
    stats: Stats,
    cache: Box<dyn Cache + 'a>,
}

impl<'a> Drop for Parser<'a> {
    fn drop(&mut self) {
        let stats = std::mem::take(&mut self.stats);
        self.parent_stats.lock().unwrap().combine(stats);
    }
}

impl<'a> Parser<'a> {
    pub fn new(parent_stats: &'a Mutex<Stats>, cache: Box<dyn 'a + Cache>) -> Parser<'a> {
        Parser {
            parent_stats,
            stats: Stats::default(),
            cache,
        }
    }

    fn on_node(&mut self, node: &Node) -> Statement {
        let info = node.info();
        let mut statement = self.process_node(
            info.deleted(),
            node.id(),
            node.tags(),
            node.lat(),
            node.lon(),
        );
        if let Statement::Create {
            ref mut val,
            ref mut ts,
            ..
        } = statement
        {
            *ts = Self::push_info(val, &info);
        }
        statement
    }

    fn on_dense_node(&mut self, node: &DenseNode) -> Statement {
        let info = node.info().unwrap();
        let mut statement = self.process_node(
            info.deleted(),
            node.id(),
            node.tags(),
            node.lat(),
            node.lon(),
        );
        if let Statement::Create {
            val: ref mut value, ..
        } = statement
        {
            value.push_metadata(
                info.version(),
                info.user().unwrap(),
                info.milli_timestamp(),
                info.changeset(),
            );
        }
        statement
    }

    fn process_node<'t, TTags: Iterator<Item = (&'t str, &'t str)> + ExactSizeIterator>(
        &mut self,
        is_deleted: bool,
        id: i64,
        tags: TTags,
        lat: f64,
        lon: f64,
    ) -> Statement {
        if is_deleted {
            self.stats.deleted_nodes += 1;
            Statement::Delete {
                elem: Element::Node,
                id,
            }
        } else {
            self.cache.set_lat_lon(id as usize, lat, lon);
            let mut value = StringBuf::new(100000);
            self.push_all_tags(&mut value, tags);
            if value.is_empty() {
                self.stats.skipped_nodes += 1;
                Statement::Skip
            } else {
                value.push_point("osmm:loc", lat, lon);
                Self::push_elem_type(&mut value, Element::Node);
                self.stats.added_nodes += 1;
                Statement::Create {
                    elem: Element::Node,
                    id,
                    val: value,
                    ts: 0,
                }
            }
        }
    }

    fn on_way(&mut self, way: &Way) -> Statement {
        let info = way.info();
        if info.deleted() {
            self.stats.deleted_ways += 1;
            return Statement::Delete {
                elem: Element::Way,
                id: way.id(),
            };
        }
        let mut value = StringBuf::new(100000);
        self.push_all_tags(&mut value, way.tags());
        Self::push_elem_type(&mut value, Element::Way);
        let ts = Self::push_info(&mut value, &info);
        if let Err(err) = self.parse_way_geometry(&mut value, way) {
            value.push_tag("osmm:loc:error", &err.to_string())
        }

        self.stats.added_ways += 1;
        Statement::Create {
            elem: Element::Way,
            id: way.id(),
            val: value,
            ts,
        }
    }

    fn on_relation(&mut self, rel: &Relation) -> Statement {
        let info = rel.info();
        if info.deleted() {
            self.stats.deleted_rels += 1;
            return Statement::Delete {
                elem: Element::Way,
                id: rel.id(),
            };
        }
        let mut value = StringBuf::new(100000);
        self.push_all_tags(&mut value, rel.tags());
        Self::push_elem_type(&mut value, Element::Relation);
        let ts = Self::push_info(&mut value, &info);
        for mbr in rel.members() {
            // Produce two statements - one to find all members of a relation,
            // and another to find the role of that relation
            //     osmrel:123  osmm:has    osmway:456
            //     osmrel:123  osmway:456  "inner"
            let mbr_prefix = match mbr.member_type {
                RelMemberType::Node => "osmnode:",
                RelMemberType::Way => "osmway:",
                RelMemberType::Relation => "osmrel:",
            };
            let mbr_id = mbr.member_id.to_string();

            value.push_str("osmm:has ");
            value.push_str(mbr_prefix);
            value.push_str(&mbr_id);
            value.push_str(";\n");
            value.push_str(mbr_prefix);
            value.push_str(&mbr_id);
            value.push(' ');
            value.push_quoted_str(mbr.role().unwrap());
        }

        self.stats.added_rels += 1;
        Statement::Create {
            elem: Element::Relation,
            id: rel.id(),
            val: value,
            ts,
        }
    }

    fn push_info(value: &mut StringBuf, info: &Info) -> i64 {
        let ts = info.milli_timestamp().unwrap();
        value.push_metadata(
            info.version().unwrap(),
            info.user().unwrap().unwrap(),
            ts,
            info.changeset().unwrap(),
        );
        ts
    }

    fn push_elem_type(value: &mut StringBuf, element: Element) {
        value.push_char_value(
            "osmm:type",
            match element {
                Element::Node => 'n',
                Element::Way => 'w',
                Element::Relation => 'r',
            },
        );
    }

    fn parse_way_geometry(&self, value: &mut StringBuf, way: &Way) -> anyhow::Result<()> {
        let refs: Vec<[f64; 2]> = way
            .refs()
            .map(|id| {
                let (lat, lng) = self.cache.get_lat_lon(id as usize);
                [lat, lng]
            })
            .collect();

        let geometry = Geometry::create_line_string(CoordSeq::new_from_vec(&refs)?)?;
        value.push_bool_value("osmm:isClosed", geometry.is_closed()?);
        let g = geometry.point_on_surface()?;
        value.push_point("osmm:loc", g.get_y().unwrap(), g.get_x().unwrap());

        Ok(())
    }

    fn push_all_tags<'t, TTags: Iterator<Item = (&'t str, &'t str)> + ExactSizeIterator>(
        &mut self,
        value: &mut StringBuf,
        tags: TTags,
    ) {
        for (k, v) in tags {
            if k != "created_by" {
                value.push_tag(k, v);
            }
        }
    }
}

fn create_flat_cache(filename: PathBuf) -> anyhow::Result<DenseFileCache> {
    Ok(DenseFileCacheOpts::new(filename)
        .page_size(10 * 1024 * 1024 * 1024)
        .on_size_change(Some(|old_size, new_size| {
            println!(
                "Growing cache {} ➡ {}",
                ByteSize(old_size as u64),
                ByteSize(new_size as u64)
            )
        }))
        .open()?)
}

fn start_writer_thread(
    output_dir: &Path,
    max_file_size: usize,
    receiver: Receiver<Statement>,
) -> JoinHandle<()> {
    let output_dir = output_dir.to_path_buf();
    let file_index = AtomicU32::new(0);
    let oldest_ts = AtomicI64::new(0);

    Builder::new()
        .name("gz_writer".into())
        .spawn(move || {
            let mut encoder = None;
            let mut size = 0_usize;
            while let Ok(v) = receiver.recv() {
                if let Statement::Create { elem, id, val, ts } = v {
                    oldest_ts.fetch_max(ts, Ordering::Relaxed);

                    let enc = encoder.get_or_insert_with(|| new_gz_file(&output_dir, &file_index));
                    writeln!(enc, "{elem}:{id}\n{val}").unwrap();

                    size += val.len();
                    if size > max_file_size {
                        encoder.take().unwrap().finish().unwrap();
                        size = 0;
                    }
                }
            }

            let mut enc = new_gz_file(&output_dir, &file_index);
            let ts = to_utc(oldest_ts.load(Ordering::SeqCst));
            writeln!(enc, "osmroot: schema:dateModified {ts}.").unwrap();
        })
        .unwrap()
}

fn new_gz_file(output_dir: &Path, file_index: &AtomicU32) -> GzEncoder<File> {
    let index = file_index.fetch_add(1, Ordering::Relaxed);
    let file = output_dir.join(format!("osm-{index:06}.ttl.gz"));
    println!("Creating {:?}", file.absolutize().unwrap());
    GzEncoder::new(File::create(file).unwrap(), Compression::default())
}

pub fn parse(opt: Args) -> anyhow::Result<()> {
    match opt.cmd {
        Command::Parse {
            workers,
            input_file,
            output_dir,
            max_file_size,
        } => {
            if let Some(v) = workers {
                rayon::ThreadPoolBuilder::new()
                    .thread_name(|i| format!("parser #{i}"))
                    .num_threads(v)
                    .build_global()
                    .unwrap();
            }
            let (sender, receiver) = channel();
            let writer_thread =
                start_writer_thread(&output_dir, max_file_size * 1024 * 1024, receiver);

            let reader = BlobReader::from_path(input_file)?;
            if let Some(filename) = &opt.planet_cache {
                let cache = create_flat_cache(filename.clone())?;
                parse_with_cache(cache, sender, reader);
            } else {
                let filename = &opt.small_cache.unwrap();
                let cache = if filename.exists() {
                    HashMapCache::from_bin(filename)?
                } else {
                    HashMapCache::new()
                };
                parse_with_cache(cache.clone(), sender, reader);
                cache.save_as_bin(filename)?;
            }

            writer_thread.join().unwrap();
            Ok(())
        } // _ => panic!("Expecting Parse")
    }
}

pub fn parse_with_cache<R: Read + Send, C: CacheStore + Clone + Send>(
    cache: C,
    sender: Sender<Statement>,
    reader: BlobReader<R>,
) {
    let stats = Mutex::new(Stats::default());
    reader
        .par_bridge()
        .for_each_with((cache, sender), |(dfc, writer), blob| {
            if let BlobDecode::OsmData(block) = blob.unwrap().decode().unwrap() {
                let mut parser = Parser::new(&stats, dfc.get_accessor());
                parse_block(block, &mut parser, |s| writer.send(s).unwrap());
            };
        });
    println!("{:#?}", stats.lock().unwrap());
}

pub fn parse_block(block: PrimitiveBlock, parser: &mut Parser, mut writer: impl FnMut(Statement)) {
    for group in block.groups() {
        // FIXME: possible concurrency bug: a non-node element may need coords of a node that hasn't been processed yet
        for node in group.nodes() {
            writer(parser.on_node(&node));
        }
        for node in group.dense_nodes() {
            writer(parser.on_dense_node(&node));
        }
        for way in group.ways() {
            writer(parser.on_way(&way));
        }
        for rel in group.relations() {
            writer(parser.on_relation(&rel));
        }
    }
}
