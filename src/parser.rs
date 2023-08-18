use std::fs::File;
use std::io::{Read, Write as _};
use std::mem;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::thread::{Builder, JoinHandle};

use bytesize::ByteSize;
use flate2::write::GzEncoder;
use flate2::Compression;
use geo::{Centroid, LineString};
use log::{info, warn};
use osmnodecache::{Cache, CacheStore, DenseFileCache, DenseFileCacheOpts, HashMapCache};
use osmpbf::{BlobDecode, BlobReader, DenseNode, Node, PrimitiveBlock, Relation, Way};
use path_absolutize::Absolutize as _;
use rayon::iter::{ParallelBridge as _, ParallelIterator as _};

use crate::str_builder::{
    StringBuf, XsdBoolean, XsdDateTime, XsdElement, XsdPoint, XsdRelMember, XsdStr,
};
use crate::utils::{Element, ElementInfo, Stats};
use crate::{Args, Command};

//noinspection HttpUrlsUsage
static PREFIXES: &[&str] = &[
    // Wikidata
    "prefix wd: <http://www.wikidata.org/entity/>",
    "prefix xsd: <http://www.w3.org/2001/XMLSchema#>",
    "prefix geo: <http://www.opengis.net/ont/geosparql#>",
    "prefix schema: <http://schema.org/>",
    // OSM
    "prefix osmroot: <https://www.openstreetmap.org>",
    "prefix osmnode: <https://www.openstreetmap.org/node/>",
    "prefix osmway: <https://www.openstreetmap.org/way/>",
    "prefix osmrel: <https://www.openstreetmap.org/relation/>",
    "prefix osmt: <https://wiki.openstreetmap.org/wiki/Key:>",
    "prefix osmm: <https://www.openstreetmap.org/meta/>",
];

#[derive(Debug)]
pub enum Statement {
    Skip,
    Delete {
        elem: Element,
        id: i64,
    },
    Create {
        elem: Element,
        id: i64,
        ts: i64,
        val: StringBuf,
    },
}

pub struct Parser<'a> {
    parent_stats: &'a Mutex<Stats>,
    stats: Stats,
    cache: Box<dyn Cache + 'a>,
    batch_size: usize,
}

impl<'a> Drop for Parser<'a> {
    fn drop(&mut self) {
        let stats = std::mem::take(&mut self.stats);
        self.parent_stats.lock().unwrap().combine(stats);
    }
}

impl<'a> Parser<'a> {
    pub fn new(
        parent_stats: &'a Mutex<Stats>,
        cache: Box<dyn 'a + Cache>,
        batch_size: usize,
    ) -> Parser<'a> {
        Parser {
            parent_stats,
            stats: Stats::default(),
            cache,
            batch_size,
        }
    }

    pub fn parse_block(&mut self, block: PrimitiveBlock, mut writer: impl FnMut(Vec<Statement>)) {
        let batch_size = self.batch_size;
        let mut result: Vec<Statement> = Vec::with_capacity(batch_size);
        let mut enqueue = |s: Statement| {
            result.push(s);
            if result.len() > batch_size {
                writer(mem::replace(&mut result, Vec::with_capacity(batch_size)));
            }
        };

        for group in block.groups() {
            // FIXME: possible concurrency bug: a non-node element may need coords of a node that hasn't been processed yet
            for node in group.nodes() {
                enqueue(self.on_node(&node));
            }
            for node in group.dense_nodes() {
                enqueue(self.on_dense_node(&node));
            }
            for way in group.ways() {
                enqueue(self.on_way(&way));
            }
            for rel in group.relations() {
                enqueue(self.on_relation(&rel));
            }
        }

        if !result.is_empty() {
            writer(result);
        }
    }

    fn on_node(&mut self, node: &Node) -> Statement {
        let info = node.info().into();
        self.process_node(info, node.id(), node.tags(), node.lat(), node.lon())
    }

    fn on_dense_node(&mut self, node: &DenseNode) -> Statement {
        let info = node.info().unwrap().into();
        self.process_node(info, node.id(), node.tags(), node.lat(), node.lon())
    }

    fn process_node<'t, TTags: Iterator<Item = (&'t str, &'t str)> + ExactSizeIterator>(
        &mut self,
        info: ElementInfo<'_>,
        id: i64,
        tags: TTags,
        lat: f64,
        lon: f64,
    ) -> Statement {
        if info.is_deleted {
            self.stats.deleted_nodes += 1;
            Statement::Delete {
                elem: Element::Node,
                id,
            }
        } else {
            self.cache.set_lat_lon(id as usize, lat, lon);
            let mut value = StringBuf::default();
            value.add_tags(tags);
            if value.is_empty() {
                self.stats.skipped_nodes += 1;
                Statement::Skip
            } else {
                value.add_value("osmm:loc", XsdPoint { lat, lon });
                value.add_value("osmm:type", XsdElement(Element::Node));
                self.stats.added_nodes += 1;
                Statement::Create {
                    elem: Element::Node,
                    id,
                    ts: info.milli_timestamp,
                    val: value.finalize(info),
                }
            }
        }
    }

    fn on_way(&mut self, way: &Way) -> Statement {
        let info: ElementInfo = way.info().into();
        if info.is_deleted {
            self.stats.deleted_ways += 1;
            return Statement::Delete {
                elem: Element::Way,
                id: way.id(),
            };
        }
        let mut value = StringBuf::default();
        value.add_tags(way.tags());
        value.add_value("osmm:type", XsdElement(Element::Way));
        if let Err(err) = self.parse_way_geometry(&mut value, way) {
            value.add_value("osmm:loc:error", XsdStr(&err.to_string()));
        }

        self.stats.added_ways += 1;
        Statement::Create {
            elem: Element::Way,
            id: way.id(),
            ts: info.milli_timestamp,
            val: value.finalize(info),
        }
    }

    fn on_relation(&mut self, rel: &Relation) -> Statement {
        let info: ElementInfo = rel.info().into();
        if info.is_deleted {
            self.stats.deleted_rels += 1;
            return Statement::Delete {
                elem: Element::Relation,
                id: rel.id(),
            };
        }

        let mut value = StringBuf::default();
        value.add_tags(rel.tags());
        value.add_value("osmm:type", XsdElement(Element::Relation));

        for mbr in rel.members() {
            // Produce two statements - one to find all members of a relation,
            // and another to find the role of that relation
            //     osmrel:123  osmm:has    osmway:456
            //     osmrel:123  osmway:456  "inner"    (this is added only if non-empty)
            value.add_value("osmm:has", XsdRelMember(&mbr));
            let role = mbr.role().unwrap();
            if !role.is_empty() {
                value.add_value(XsdRelMember(&mbr), XsdStr(role));
            }
        }

        self.stats.added_rels += 1;
        Statement::Create {
            elem: Element::Relation,
            id: rel.id(),
            ts: info.milli_timestamp,
            val: value.finalize(info),
        }
    }

    fn parse_way_geometry(&self, value: &mut StringBuf, way: &Way) -> anyhow::Result<()> {
        let geometry: LineString = way
            .refs()
            .map(|id| {
                let (lat, lng) = self.cache.get_lat_lon(id as usize);
                [lat, lng]
            })
            .collect();

        let value1 = geometry.is_closed();
        value.add_value("osmm:isClosed", XsdBoolean(value1));

        if let Some(g) = geometry.centroid() {
            let point = XsdPoint {
                lat: g.y(),
                lon: g.x(),
            };
            value.add_value("osmm:loc", point);
        }

        Ok(())
    }
}

fn create_flat_cache(filename: PathBuf) -> anyhow::Result<DenseFileCache> {
    Ok(DenseFileCacheOpts::new(filename)
        .page_size(10 * 1024 * 1024 * 1024)
        .on_size_change(Some(|old_size, new_size| {
            info!(
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
    receiver: Receiver<Vec<Statement>>,
) -> JoinHandle<()> {
    let output_dir = output_dir.to_path_buf();
    let file_index = AtomicU32::new(0);
    let oldest_ts = AtomicI64::new(0);

    Builder::new()
        .name("gz_writer".into())
        .spawn(move || {
            let mut encoder = None;
            let mut size = 0_usize;
            while let Ok(batch) = receiver.recv() {
                for statement in batch {
                    match statement {
                        Statement::Create { elem, id, val, ts } => {
                            oldest_ts.fetch_max(ts, Ordering::Relaxed);

                            let enc = encoder
                                .get_or_insert_with(|| new_gz_file(&output_dir, &file_index));
                            write!(enc, "\n{elem}:{id}\n{val}").unwrap();

                            size += val.len();
                            if size > max_file_size {
                                encoder.take().unwrap().finish().unwrap();
                                size = 0;
                            }
                        }
                        Statement::Skip => {}
                        Statement::Delete { elem, id } => {
                            warn!("Delete {elem}:{id} is not supported");
                        }
                    }
                }
            }

            // Create a separate file with the date of the last modification
            let mut enc = new_gz_file(&output_dir, &file_index);
            let ts = XsdDateTime(oldest_ts.load(Ordering::SeqCst));
            writeln!(enc, "\nosmroot: schema:dateModified {ts}.").unwrap();
        })
        .unwrap()
}

fn new_gz_file(output_dir: &Path, file_index: &AtomicU32) -> GzEncoder<File> {
    let index = file_index.fetch_add(1, Ordering::Relaxed);
    let filename = output_dir.join(format!("osm-{index:06}.ttl.gz"));
    info!("Creating {:?}", filename.absolutize().unwrap());
    let file = File::create(filename).unwrap();
    let mut enc = GzEncoder::new(file, Compression::default());
    for prefix in PREFIXES {
        writeln!(enc, "@{prefix}.").unwrap();
    }
    enc
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
            let stats = if let Some(filename) = &opt.planet_cache {
                info!("Creating dense cache in {:?}", filename.display());
                let cache = create_flat_cache(filename.clone())?;
                parse_with_cache(cache, sender, reader)
            } else {
                let cache = if let Some(filename) = &opt.small_cache {
                    if filename.exists() {
                        info!("Loading sparse cache from {:?}", filename.display());
                        HashMapCache::from_bin(filename)?
                    } else {
                        HashMapCache::new()
                    }
                } else {
                    HashMapCache::new()
                };

                let stats = parse_with_cache(cache.clone(), sender, reader);

                if let Some(filename) = &opt.small_cache {
                    info!("Saving sparse cache to {:?}", filename.display());
                    cache.save_as_bin(filename)?;
                }

                stats
            };

            writer_thread.join().unwrap();
            info!("Run statistics:\n{stats:#?}");
            Ok(())
        }
    }
}

pub fn parse_with_cache<R: Read + Send, C: CacheStore + Clone + Send>(
    cache: C,
    sender: Sender<Vec<Statement>>,
    reader: BlobReader<R>,
) -> Stats {
    let stats = Mutex::new(Stats::default());
    reader
        .par_bridge()
        .for_each_with((cache, sender), |(dfc, sender), blob| {
            if let BlobDecode::OsmData(block) = blob.unwrap().decode().unwrap() {
                let mut parser = Parser::new(&stats, dfc.get_accessor(), 1024);
                parser.parse_block(block, |s| sender.send(s).unwrap());
            };
        });
    stats.into_inner().unwrap()
}
