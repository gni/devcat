use crate::{error::Result, history::History, utils, OutputArgs, ExcludeArgs};
use clap::Args;
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use log::debug;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Args, Debug)]
#[command(about = "The default command. Concatenates files from a directory or a snapshot.")]
pub struct CatArgs {
    /// The path to concatenate. Defaults to the current directory.
    pub path: Option<PathBuf>,
    /// Concatenate from a snapshot ID instead of the filesystem.
    #[arg(long, short)]
    pub id: Option<u32>,
    #[command(flatten)]
    pub exclude_args: ExcludeArgs,
    #[command(flatten)]
    pub output_args: OutputArgs,
}

pub fn run(args: CatArgs) -> Result<()> {
    let mut output = String::new();
    let root_path = Path::new(".");
    
    // Load config and merge excludes
    let config = crate::config::load_config(root_path)?;
    let mut excludes = args.exclude_args.exclude.clone();
    excludes.extend(config.exclude);

    if let Some(id) = args.id {
        cat_from_snapshot(id, root_path, &mut output)?;
    } else {
        let path = args.path.unwrap_or_else(|| PathBuf::from("."));
        cat_from_workdir(&path, &excludes, &mut output)?;
    }

    utils::handle_output(output, &args.output_args, "File content")
}

fn cat_from_snapshot(id: u32, root_path: &Path, output: &mut String) -> Result<()> {
    debug!("Concatenating files from snapshot ID {}", id);
    let history = History::load(root_path)?;
    let snapshot = history.get_snapshot(id)?;
    
    let manifest: BTreeMap<PathBuf, String> = utils::get_manifest_from_hash(root_path, &snapshot.manifest_hash)?;

    for (path, hash) in manifest {
        let objects_dir = root_path.join(".devcat").join("objects");
        match fs::read_to_string(objects_dir.join(hash)) {
            Ok(content) => {
                writeln!(output, "--- START FILE: {} ---", path.display())?;
                writeln!(output, "{}", content)?;
                writeln!(output, "--- END FILE: {} ---\n", path.display())?;
            }
            Err(_) => {
                writeln!(output, "--- START FILE: {} ---", path.display())?;
                writeln!(output, "[Could not read object as text]")?;
                writeln!(output, "--- END FILE: {} ---\n", path.display())?;
            }
        }
    }
    Ok(())
}

fn cat_from_workdir(path: &Path, excludes: &[String], output: &mut String) -> Result<()> {
    let mut skipped_items = Vec::new();

    let mut glob_builder = GlobSetBuilder::new();
    for pattern in excludes {
        let glob = Glob::new(&format!("**/{}", pattern))?;
        glob_builder.add(glob);
    }
    let exclude_set = glob_builder.build()?;

    let walker = WalkBuilder::new(path)
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
        let entry = match result {
            Ok(entry) => entry,
            Err(e) => {
                let path_str = match &e {
                    ignore::Error::WithPath { path, .. } => path.to_string_lossy().to_string(),
                    _ => "[Unknown Path]".to_string(),
                };
                skipped_items.push((path_str, e.to_string()));
                continue;
            }
        };

        let current_path = entry.path();
        if current_path.is_file() {
            if utils::check_file_signature(current_path)? {
                debug!("Skipping devcat output file: {}", current_path.display());
                continue;
            }
            if let Err(e) = utils::append_file_content(
                current_path,
                current_path.strip_prefix(path).unwrap_or(current_path),
                output,
            ) {
                skipped_items.push((current_path.to_string_lossy().to_string(), e.to_string()));
            }
        }
    }

    if !skipped_items.is_empty() {
        eprintln!("\n⚠️ The following paths were skipped due to errors:");
        for (path, error) in skipped_items {
            eprintln!("- {}: {}", path, error);
        }
    }

    Ok(())
}
