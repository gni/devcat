use clap::{Args, Parser};
use std::path::PathBuf;
mod cat;
mod clean;
mod config;
mod diff;
mod error;
mod history;
mod inspect;
mod log_cmd;
mod module;
mod prune;
mod revert;
mod save;
mod trace;
mod utils;
mod watch;

use log::{debug, LevelFilter};

pub use error::Result;

#[derive(Parser, Debug)]
#[command(
    name = "devcat",
    version = "0.1.1",
    about = "A self-contained snapshot and context tool for your development loop.",
    long_about = "devcat creates filesystem-based snapshots of your work, allowing you to diff, view, and revert to specific checkpoints in any directory."
)]
struct Cli {
    #[command(flatten)]
    cat_args: cat::CatArgs,

    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Args, Debug, Clone)]
pub struct OutputArgs {
    #[arg(short, long, global = true)]
    pub output: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct ExcludeArgs {
    #[arg(short = 'e', long)]
    pub exclude: Vec<String>,
}

#[derive(Parser, Debug)]
enum Commands {
    Save(save::SaveArgs),
    Revert(revert::RevertArgs),
    Log(log_cmd::LogArgs),
    Diff(diff::DiffArgs),
    Module(module::ModuleArgs),
    Trace(trace::TraceArgs),
    Clean(clean::CleanArgs),
    Prune(prune::PruneArgs),
    Inspect(inspect::InspectArgs),
    Watch(watch::WatchArgs),
}

fn main() {
    let cli = Cli::parse();

    let log_level = match cli.verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        _ => LevelFilter::Debug,
    };
    env_logger::Builder::new().filter_level(log_level).init();

    debug!("CLI arguments parsed successfully.");

    let result = match cli.command {
        Some(Commands::Save(args)) => save::run(args),
        Some(Commands::Revert(args)) => revert::run(args),
        Some(Commands::Log(args)) => log_cmd::run(args),
        Some(Commands::Diff(args)) => diff::run(args),
        Some(Commands::Module(args)) => module::run(args),
        Some(Commands::Trace(args)) => trace::run(args),
        Some(Commands::Clean(args)) => clean::run(args),
        Some(Commands::Prune(args)) => prune::run(args),
        Some(Commands::Inspect(args)) => inspect::run(args),
        Some(Commands::Watch(args)) => watch::run(args),
        None => cat::run(cli.cat_args),
    };

    if let Err(e) = result {
        log::error!("{}", e);
        std::process::exit(1);
    }
}
