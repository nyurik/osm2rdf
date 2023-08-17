use clap::Parser;
use osm2rdf::{parser, Args, Command};

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.cmd {
        Command::Parse { .. } => parser::parse(args),
    }
}
