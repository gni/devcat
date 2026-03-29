use crate::{history::History, utils, Result, OutputArgs, ExcludeArgs};
use clap::Parser;
use similar::{ChangeTag, TextDiff};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(about = "Diffs between snapshots or against the working directory.")]
pub struct DiffArgs {
    /// The first snapshot ID to compare. Defaults to the latest snapshot.
    pub id1: Option<u32>,
    /// The second snapshot ID to compare. If omitted, compares ID1 to the working directory.
    pub id2: Option<u32>,
    #[command(flatten)]
    pub output_args: OutputArgs,
    #[command(flatten)]
    pub exclude_args: ExcludeArgs,
}

pub fn run(args: DiffArgs) -> Result<()> {
    let root_path = Path::new(".");
    let history = History::load(root_path)?;
    let mut output = String::new();

    let mut excludes = args.exclude_args.exclude.clone();
    // Automatically exclude the output file itself to prevent self-inclusion
    if let Some(output_path) = &args.output_args.output {
        if let Some(file_name) = output_path.file_name() {
            excludes.push(file_name.to_string_lossy().to_string());
        }
    }

    let (old_manifest, new_manifest) = match (args.id1, args.id2) {
        (Some(id1), Some(id2)) => (get_manifest(id1, &history)?, get_manifest(id2, &history)?),
        (Some(id), None) => (get_manifest(id, &history)?, utils::get_current_manifest(root_path, &excludes)?),
        (None, None) => {
            let latest = history.get_latest()?;
            (get_manifest(latest.id, &history)?, utils::get_current_manifest(root_path, &excludes)?)
        }
        _ => return Err(crate::error::Error::Format(std::fmt::Error)),
    };

    let objects_dir = root_path.join(".devcat").join("objects");
    let all_paths: BTreeMap<_, _> = old_manifest.keys().chain(new_manifest.keys()).map(|k| (k, true)).collect();

    for path in all_paths.keys() {
        let old_hash = old_manifest.get(*path);
        let new_hash = new_manifest.get(*path);

        if old_hash == new_hash { continue; }

        let old_content = match old_hash {
            Some(hash) => fs::read_to_string(objects_dir.join(hash))?,
            None => String::new(),
        };
        
        let new_content = match new_hash {
            Some(_) => fs::read_to_string(root_path.join(path))?,
            None => String::new(),
        };

        output.push_str(&generate_diff(path, &old_content, &new_content));
    }
    
    if output.is_empty() {
        println!("✅ No changes detected since last snapshot.");
    } else {
        utils::handle_output(output, &args.output_args, "Diff")?;
    }
    
    Ok(())
}

fn get_manifest(id: u32, history: &History) -> Result<BTreeMap<PathBuf, String>> {
    let snapshot = history.get_snapshot(id)?;
    let objects_dir = Path::new(".").join(".devcat").join("objects");
    let manifest_content = fs::read(objects_dir.join(&snapshot.manifest_hash))?;
    serde_json::from_slice(&manifest_content).map_err(Into::into)
}

fn generate_diff(path: &Path, old: &str, new: &str) -> String {
    let mut diff_text = format!("--- a/{}\n+++ b/{}\n", path.display(), path.display());
    let diff = TextDiff::from_lines(old, new);
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        diff_text.push_str(&format!("{}{}", sign, change));
    }
    diff_text
}
