use crate::error::Result;
use clap::Args;
use std::path::{Path, PathBuf};
use std::io::Read;

#[derive(Args, Debug)]
#[command(name = "split")]
pub struct SplitArgs {
    /// path to source text file (.txt/.md/any). If omitted, reads from stdin.
    #[arg(short, long)]
    pub input: Option<PathBuf>,

    /// output directory root
    #[arg(short = 'd', long, default_value = ".")]
    pub outdir: PathBuf,

    /// parsing mode
    #[arg(long, default_value = "fenced", value_parser = ["fenced", "loose"])]
    pub mode: String,

    /// allow overwriting existing files
    #[arg(short = 'F', long)]
    pub overwrite: bool,

    /// show actions without writing files
    #[arg(long)]
    pub dry_run: bool,

    /// text encoding for reading/writing
    #[arg(long, default_value = "utf-8")]
    pub encoding: String,
}

pub fn run(args: SplitArgs) -> Result<()> {
    let text = match &args.input {
        Some(src) => {
            if !src.exists() || !src.is_file() {
                return Err(crate::error::Error::Custom(format!("error: input_not_found, path={}", src.display())).into());
            }
            std::fs::read_to_string(src)?
        }
        None => {
            let mut buffer = String::new();
            std::io::stdin().read_to_string(&mut buffer)?;
            buffer
        }
    };

    execute_split(&text, &args.outdir, &args.mode, args.overwrite, args.dry_run, false)
}

pub fn execute_split(text: &str, outdir_arg: &Path, _mode: &str, overwrite: bool, dry_run: bool, skip_patches: bool) -> Result<()> {
    let outdir = outdir_arg.canonicalize().unwrap_or_else(|_| outdir_arg.to_path_buf());
    std::fs::create_dir_all(&outdir)?;

    let normalized_text = text.replace("\r\n", "\n");
    let sections = parse_sections(&normalized_text, _mode);

    if sections.is_empty() {
        return Err(crate::error::Error::Custom("error: no_valid_file_sections_found. ensure_filename_precedes_code_block".to_string()).into());
    }

    let mut written = Vec::new();
    for (rel, content) in sections {
        if skip_patches && content.contains("<<<< SEARCH") && content.contains(">>>> REPLACE") {
            continue;
        }

        let safe_rel = normalize_relpath(&rel);
        let dest = outdir.join(&safe_rel);

        if dry_run {
            log::info!("plan_write, path={}, bytes={}", dest.display(), content.len());
            written.push(dest.to_string_lossy().to_string());
            continue;
        }

        if dest.exists() && !overwrite {
            log::warn!("warning: file_exists_skipping, path={}", dest.display());
            continue;
        }

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&dest, content.as_bytes())?;
        log::info!("wrote_file, path={}, bytes={}", dest.display(), content.len());
        written.push(dest.to_string_lossy().to_string());
    }

    log::info!("summary, files={}, dry_run={}, outdir={}", written.len(), dry_run, outdir.display());
    Ok(())
}

fn parse_sections(text: &str, _mode: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut sections = Vec::new();
    let mut i = 0;
    let n = lines.len();

    while i < n {
        let line = lines[i].trim();
        
        // If we find a code block without a header, we alert/skip
        if detect_fence(line).is_some() {
            log::error!("error: orphan_code_block_detected, line={}, status=skipping_no_filename", i + 1);
            // Skip this entire block
            let fence_char = detect_fence(line).unwrap();
            i += 1;
            while i < n && detect_fence(lines[i]) != Some(fence_char) {
                i += 1;
            }
            i += 1;
            continue;
        }

        if !line.is_empty() {
            // Potential header. Check next non-empty line.
            let mut j = i + 1;
            while j < n && lines[j].trim().is_empty() {
                j += 1;
            }

            if j < n {
                if let Some(fence_char) = detect_fence(lines[j]) {
                    let header = line.to_string();
                    let content_start = j + 1;
                    let mut k = content_start;
                    let mut content_end = None;

                    while k < n {
                        if detect_fence(lines[k]) == Some(fence_char) {
                            content_end = Some(k);
                            break;
                        }
                        k += 1;
                    }

                    if let Some(end) = content_end {
                        let content = lines[content_start..end].join("\n");
                        sections.push((header, content));
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    sections
}

fn detect_fence(line: &str) -> Option<char> {
    let trimmed = line.trim();
    if trimmed.starts_with("```") { Some('`') }
    else if trimmed.starts_with("~~~") { Some('~') }
    else { None }
}

fn normalize_relpath(p: &str) -> String {
    let mut p = p.trim().trim_matches(|c| c == '\'' || c == '"').replace('\\', "/");
    while p.starts_with('/') { p = p[1..].to_string(); }
    if let Some(normalized) = normalize_path(&p) { p = normalized; }
    p
}

fn normalize_path(p: &str) -> Option<String> {
    let mut parts = Vec::new();
    for component in p.split('/') {
        if component == ".." { parts.pop(); }
        else if component != "." && !component.is_empty() { parts.push(component); }
    }
    if parts.is_empty() { Some(".".to_string()) } else { Some(parts.join("/")) }
}
