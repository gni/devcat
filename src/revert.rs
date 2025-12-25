use crate::{history::History, Result};
use clap::Parser;
use ignore::WalkBuilder;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(about = "Reverts the working directory to a specific snapshot state.")]
pub struct RevertArgs {
    /// The snapshot ID to revert the working directory to.
    #[arg(required = true)]
    pub id: u32,
}

pub fn run(args: RevertArgs) -> Result<()> {
    let root_path = Path::new(".");
    let history = History::load(root_path)?;
    let snapshot = history.get_snapshot(args.id)?;

    let objects_dir = root_path.join(".devcat").join("objects");
    let manifest_content = fs::read(objects_dir.join(&snapshot.manifest_hash))?;
    let manifest: BTreeMap<PathBuf, String> = serde_json::from_slice(&manifest_content)?;

    // Restore phase: Overwrite existing files or create new ones from the snapshot.
    for (path, hash) in &manifest {
        let content = fs::read(objects_dir.join(hash))?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(path, content)?;
    }

    let manifest_paths: HashSet<_> = manifest.keys().collect();

    // Cleanup phase: Identify files in workspace that are not in the snapshot.
    // Also collect directories to attempt cleanup later.
    let mut walker_builder = WalkBuilder::new(root_path);
    walker_builder.filter_entry(|entry| !entry.path().starts_with("./.devcat"));
    let walker = walker_builder.build();

    let mut directories_to_clean = Vec::new();

    for result in walker {
        let entry = result?;
        let path = entry.path();
        
        if path.is_dir() {
            if path != root_path {
                directories_to_clean.push(path.to_path_buf());
            }
        } else if path.is_file() {
            if let Ok(relative_path) = path.strip_prefix(root_path) {
                // If the file exists in the workspace but not the snapshot manifest, nuke it.
                if !manifest_paths.contains(&relative_path.to_path_buf()) {
                    fs::remove_file(path)?;
                }
            }
        }
    }

    // Directory Pruning: `ignore` crate doesn't support post-order traversal out of the box.
    // Sort directories by path length descending (deepest first) to ensure children are deleted before parents.
    directories_to_clean.sort_by(|a, b| b.as_os_str().len().cmp(&a.as_os_str().len()));

    for dir in directories_to_clean {
        // Optimistic delete: only succeeds if directory is empty (no ignored files, no leftover artifacts).
        // Silence the error because a non-empty directory is a valid state (user might have untracked files).
        let _ = fs::remove_dir(dir);
    }
    
    println!("✅ Reverted working directory to snapshot {}.", args.id);
    Ok(())
}
