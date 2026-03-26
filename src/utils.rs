use crate::{error::{Error, Result}, history::History, OutputArgs};
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use log::debug;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const FILE_SIGNATURE: &str = "// DEVCAT-OUTPUT-FILE";
pub const HISTORY_DIR: &str = ".devcat";

#[derive(Debug, PartialEq, Eq)]
pub enum SaveStatus {
    Saved { id: u32, message: String },
    NoChanges,
}

pub fn hash_content(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    format!("{:x}", hasher.finalize())
}

pub fn handle_output(content: String, output_args: &OutputArgs, context_name: &str) -> Result<()> {
    if let Some(path) = &output_args.output {
        debug!("Writing output to file: {}", path.display());
        let final_content = format!("{}\n{}", FILE_SIGNATURE, content);
        fs::write(path, final_content)?;
        println!("✅ {} context saved to {}", context_name, path.display());
    } else {
        print!("{}", content);
    }
    Ok(())
}

pub fn check_file_signature(path: &Path) -> Result<bool> {
    let mut file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(false),
    };
    let signature_bytes = FILE_SIGNATURE.as_bytes();
    let mut buffer = vec![0; signature_bytes.len()];
    match Read::read_exact(&mut file, &mut buffer) {
        Ok(_) => Ok(buffer == signature_bytes),
        Err(_) => Ok(false),
    }
}

pub fn append_file_content(
    full_path: &Path,
    relative_path: &Path,
    output: &mut String,
) -> Result<()> {
    use std::fmt::Write;
    writeln!(output, "{}", relative_path.display())?;
    let ext = relative_path.extension().map(|e| e.to_string_lossy()).unwrap_or_default();
    writeln!(output, "```{}", ext)?;
    match fs::read_to_string(full_path) {
        Ok(content) => writeln!(output, "{}", content)?,
        Err(_) => writeln!(output, "[Skipped binary file]")?,
    }
    writeln!(output, "```")?;
    writeln!(output)?;
    Ok(())
}

pub fn ensure_history_dir_exists() -> Result<PathBuf> {
    let path = PathBuf::from(HISTORY_DIR);
    if !path.exists() {
        debug!("Creating history directory at {}", path.display());
        fs::create_dir_all(&path)?;
    }
    Ok(path)
}

pub fn get_current_manifest(
    root_path: &Path,
    excludes: &[String],
) -> Result<BTreeMap<PathBuf, String>> {
    let mut manifest = BTreeMap::new();
    let mut skipped_items = Vec::new();

    let mut glob_builder = GlobSetBuilder::new();
    for pattern in excludes {
        let glob = Glob::new(&format!("**/{}", pattern))?;
        glob_builder.add(glob);
    }
    let exclude_set = glob_builder.build()?;

    let walker = WalkBuilder::new(root_path)
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

        let path = entry.path();
        if path.is_file() {
            if check_file_signature(path)? {
                debug!("Skipping devcat output file: {}", path.display());
                continue;
            }
            let content = match fs::read(path) {
                Ok(content) => content,
                Err(e) => {
                    skipped_items.push((path.to_string_lossy().to_string(), e.to_string()));
                    continue;
                }
            };
            let hash = hash_content(&content);
            if let Ok(relative_path) = path.strip_prefix(root_path) {
                if !relative_path.as_os_str().is_empty() {
                    manifest.insert(relative_path.to_path_buf(), hash);
                }
            }
        }
    }

    if !skipped_items.is_empty() {
        eprintln!("\n⚠️ The following paths were skipped due to errors:");
        for (path, error) in skipped_items {
            eprintln!("- {}: {}", path, error);
        }
    }

    Ok(manifest)
}

pub fn get_manifest_from_hash(root_path: &Path, hash: &str) -> Result<BTreeMap<PathBuf, String>> {
    let objects_dir = root_path.join(HISTORY_DIR).join("objects");
    let manifest_path = objects_dir.join(hash);
    if !manifest_path.exists() {
        return Err(Error::ObjectNotFound(hash.to_string()));
    }
    let manifest_content = fs::read(manifest_path)?;
    serde_json::from_slice(&manifest_content).map_err(Into::into)
}

pub fn perform_save(root_path: &Path, message: &str, excludes: &[String]) -> Result<SaveStatus> {
    ensure_history_dir_exists()?;
    
    let objects_dir = root_path.join(HISTORY_DIR).join("objects");
    fs::create_dir_all(&objects_dir)?;

    let mut history = History::load(root_path)?;
    let manifest = get_current_manifest(root_path, excludes)?;
    let manifest_content = serde_json::to_vec(&manifest)?;
    let current_manifest_hash = hash_content(&manifest_content);

    if let Ok(latest) = history.get_latest() {
        if latest.manifest_hash == current_manifest_hash {
            return Ok(SaveStatus::NoChanges);
        }
    }

    for (path, hash) in &manifest {
        let object_path = objects_dir.join(hash);
        if !object_path.exists() {
            let content = fs::read(root_path.join(path))?;
            fs::write(object_path, content)?;
        }
    }

    fs::write(objects_dir.join(&current_manifest_hash), manifest_content)?;
    
    history.add_snapshot(message.to_string(), current_manifest_hash);
    history.save()?;

    let latest = history.get_latest()?;
    Ok(SaveStatus::Saved {
        id: latest.id,
        message: latest.message.clone(),
    })
}
