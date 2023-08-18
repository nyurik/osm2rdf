use clap::Parser;
use osm2rdf::{parser, Args, Command};

fn main() -> anyhow::Result<()> {
    let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "osm2rdf=info");
    env_logger::Builder::from_env(env).init();

    let args = Args::parse();
    match args.cmd {
        Command::Parse { .. } => parser::parse(args),
    }
}
