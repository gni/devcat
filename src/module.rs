use crate::{utils, OutputArgs, Result, ExcludeArgs};
use clap::Parser;
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use log::debug;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(about = "Concatenates all non-ignored text files in a specific module directory.")]
pub struct ModuleArgs {
    /// The directory of the module to concatenate.
    #[arg(required = true)]
    path: PathBuf,
    #[command(flatten)]
    pub exclude_args: ExcludeArgs,
    #[command(flatten)]
    output_args: OutputArgs,
}

pub fn run(args: ModuleArgs) -> Result<()> {
    let mut output = String::new();
    
    // Load config and merge excludes
    let config = crate::config::load_config(std::path::Path::new("."))?;
    let mut excludes = args.exclude_args.exclude.clone();
    excludes.extend(config.exclude);

    let mut glob_builder = GlobSetBuilder::new();
    for pattern in &excludes {
        let glob = Glob::new(&format!("**/{}", pattern))?;
        glob_builder.add(glob);
    }
    let exclude_set = glob_builder.build()?;

    let walker = WalkBuilder::new(&args.path)
        .follow_links(false)
        .filter_entry(move |entry| {
            if entry.path().starts_with("./.devcat") {
                return false;
            }
            if exclude_set.is_match(entry.path()) {
                debug!("Excluding path via --exclude: {}", entry.path().display());
                return false;
            }
            true
        })
        .build();

    for result in walker {
        let entry = result?;
        let path = entry.path();
        if path.is_file() {
            if utils::check_file_signature(path)? {
                debug!("Skipping devcat output file: {}", path.display());
                continue;
            }
            utils::append_file_content(
                path,
                path.strip_prefix(&args.path).unwrap_or(path),
                &mut output,
            )?;
        }
    }

    utils::handle_output(output, &args.output_args, "Module content")
}
