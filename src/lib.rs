#![doc = include_str!("../README.md")]

use std::path::PathBuf;

use anyhow::bail;
use clap::{Parser, Subcommand};

pub mod parser;
pub mod str_builder;
pub mod utils;

// group = ArgGroup::with_name("cache").required(true)

#[derive(Parser, Debug)]
#[command(about, version)]
pub struct Args {
    /// Enable verbose output.
    #[arg(short, long)]
    #[allow(dead_code)]
    pub verbose: bool,

    /// File for planet-size node cache.
    #[arg(short, long, group = "cache", value_name = "file")]
    pub planet_cache: Option<PathBuf>,

    /// File for node cache for small extracts.
    #[arg(short, long, group = "cache", value_name = "file")]
    pub small_cache: Option<PathBuf>,

    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Parses a PBF file into multiple .ttl.gz (Turtle files)
    Parse {
        /// Approximate maximum uncompressed file size, in MB, per output file.
        #[arg(short, long, default_value = "100")]
        max_file_size: usize,
        /// Number of worker threads to run. Defaults to number of logical CPUs.
        #[arg(short, long)]
        workers: Option<usize>,
        /// OSM input PBF file
        input_file: PathBuf,
        /// Output directory
        #[arg(value_parser = parse_outdir)]
        output_dir: PathBuf,
    },
    // /// Download OSM incremental update files and store them as either TTL files or the RDF database.
    // Update {
    //     /// Start updating from this sequence ID. By default, gets it from RDF server.
    //     #[arg(long)]
    //     seqid: Option<i64>,
    //     /// Source of the minute data.
    //     #[arg(
    //         long,
    //         default_value = "https://planet.openstreetmap.org/replication/minute"
    //     )]
    //     updater_url: String,
    //     /// Maximum size in kB for changes to download at once
    //     #[arg(long, default_value = "10240")]
    //     max_download: usize,
    //     /// Do not modify RDF database.
    //     #[arg(short, long)]
    //     dry_run: bool,
    //     /// Approximate maximum uncompressed file size, in MB, per output file. Only used if destination is a directory.
    //     #[arg(short, long, default_value = "100")]
    //     max_file_size: usize,
    //     /// Either a URL of the RDF database, or a directory with TTL files created with the "parse" command.
    //     #[arg(default_value = "http://localhost:9999/bigdata/namespace/wdq/sparql")]
    //     destination: String,
    // },
}

// enum Foo {
//     /// Host URL to upload data. Default: %(default)s
//     #[arg(
//     long,
//     default_value = "http://localhost:9999/bigdata/namespace/wdq/sparql"
//     )]
//     host: String,
// }

fn parse_outdir(path_str: &str) -> anyhow::Result<PathBuf> {
    let path = PathBuf::from(path_str);
    if !path.is_dir() {
        bail!("Output directory `{path_str}` does not exist")
    }
    Ok(path)
}
