use crate::error::{Error, Result};
use clap::Args;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const BACKTICK_FENCE: &str = "```";
const TILDE_FENCE: &str = "~~~";
const SEARCH_MARKER_4: &str = concat!("<<<<", " SEARCH");
const SEARCH_MARKER_3: &str = concat!("<<<", " SEARCH");
const REPLACE_MARKER_4: &str = concat!(">>>>", " REPLACE");
const REPLACE_MARKER_3: &str = concat!(">>>", " REPLACE");

#[derive(Args, Debug)]
#[command(name = "split", about = "Splits fenced code blocks into individual source files.")]
pub struct SplitArgs {
    /// Path to source text file (.txt/.md/any) via flag.
    #[arg(short, long)]
    pub input: Option<PathBuf>,

    /// Optional positional path to source text file. If omitted, reads from stdin.
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Output directory root.
    #[arg(short = 'd', long, default_value = ".")]
    pub outdir: PathBuf,

    /// Parsing mode.
    #[arg(long, default_value = "fenced", value_parser = ["fenced", "loose"])]
    pub mode: String,

    /// Allow overwriting existing files.
    #[arg(short = 'F', long)]
    pub overwrite: bool,

    /// Show actions without writing files.
    #[arg(long)]
    pub dry_run: bool,

    /// Text encoding for reading/writing.
    #[arg(long, default_value = "utf-8")]
    pub encoding: String,
}

pub fn run(args: SplitArgs) -> Result<()> {
    let input_path = args.input.as_ref().or(args.file.as_ref());
    let text = match input_path {
        Some(src) => {
            if !src.exists() || !src.is_file() {
                return Err(Error::Custom(format!(
                    "error: input_not_found, path={}",
                    src.display()
                )));
            }
            fs::read_to_string(src)?
        }
        None => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            buffer
        }
    };

    execute_split(
        &text,
        &args.outdir,
        &args.mode,
        args.overwrite,
        args.dry_run,
        false,
    )
}

pub fn is_patch_block(content: &str) -> bool {
    let has_search_line = content.lines().any(|line| {
        let t = line.trim();
        t == SEARCH_MARKER_4 || t == SEARCH_MARKER_3
    });
    let has_replace_line = content.lines().any(|line| {
        let t = line.trim();
        t == REPLACE_MARKER_4 || t == REPLACE_MARKER_3
    });
    let has_unified = content
        .lines()
        .any(|line| line.starts_with("@@ ") && line.contains(" @@"));

    (has_search_line && has_replace_line) || has_unified
}

pub fn execute_split(
    text: &str,
    outdir_arg: &Path,
    mode: &str,
    overwrite: bool,
    dry_run: bool,
    skip_patches: bool,
) -> Result<()> {
    let outdir = outdir_arg
        .canonicalize()
        .unwrap_or_else(|_| outdir_arg.to_path_buf());
    fs::create_dir_all(&outdir)?;

    let normalized_text = text.replace("\r\n", "\n");
    let sections = parse_sections(&normalized_text, mode);

    if sections.is_empty() {
        return Err(Error::Custom(
            "error: no_valid_file_sections_found. ensure_filename_precedes_code_block".to_string(),
        ));
    }

    let mut written = Vec::new();
    for (rel, content) in sections {
        if skip_patches && is_patch_block(&content) {
            continue;
        }

        let safe_rel = normalize_relpath(&rel);
        if safe_rel.is_empty() || safe_rel == "." {
            continue;
        }
        let dest = outdir.join(&safe_rel);

        if dry_run {
            println!(
                "🔍 [dry-run] Would write file: {} ({} bytes)",
                safe_rel,
                content.len()
            );
            log::info!("plan_write, path={}, bytes={}", dest.display(), content.len());
            written.push(dest.to_string_lossy().to_string());
            continue;
        }

        if dest.exists() && !overwrite {
            eprintln!(
                "⚠️ File exists, skipping (use -F/--overwrite to overwrite): {}",
                safe_rel
            );
            log::warn!("warning: file_exists_skipping, path={}", dest.display());
            continue;
        }

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut final_content = content;
        if !final_content.ends_with('\n') {
            final_content.push('\n');
        }

        fs::write(&dest, final_content.as_bytes())?;
        println!("✅ Wrote file: {}", safe_rel);
        log::info!(
            "wrote_file, path={}, bytes={}",
            dest.display(),
            final_content.len()
        );
        written.push(dest.to_string_lossy().to_string());
    }

    log::info!(
        "summary, files={}, dry_run={}, outdir={}",
        written.len(),
        dry_run,
        outdir.display()
    );
    Ok(())
}

pub fn parse_sections(text: &str, _mode: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut sections = Vec::new();
    let mut i = 0;
    let n = lines.len();

    while i < n {
        let line = lines[i].trim();

        if let Some(fence_char) = detect_fence(line) {
            log::warn!(
                "warning: orphan_code_block_detected, line={}, status=skipping_no_filename",
                i + 1
            );
            i += 1;
            let mut inner_open = false;
            while i < n {
                if let Some(c) = detect_fence(lines[i]) {
                    if c == fence_char {
                        let rest = fence_info_string(lines[i]);
                        if !rest.is_empty() {
                            inner_open = true;
                        } else if inner_open {
                            inner_open = false;
                        } else {
                            break;
                        }
                    }
                }
                i += 1;
            }
            i += 1;
            continue;
        }

        if !line.is_empty() && !line.starts_with(crate::utils::FILE_SIGNATURE) {
            let mut j = i + 1;
            while j < n && lines[j].trim().is_empty() {
                j += 1;
            }

            if j < n {
                if let Some(fence_char) = detect_fence(lines[j]) {
                    let header = clean_header_path(line);
                    let content_start = j + 1;
                    let mut k = content_start;
                    let mut content_end = None;
                    let mut inner_open = false;

                    while k < n {
                        if let Some(c) = detect_fence(lines[k]) {
                            if c == fence_char {
                                let rest = fence_info_string(lines[k]);
                                if !rest.is_empty() {
                                    inner_open = true;
                                } else if inner_open {
                                    inner_open = false;
                                } else {
                                    content_end = Some(k);
                                    break;
                                }
                            }
                        }
                        k += 1;
                    }

                    if let Some(end) = content_end {
                        if looks_like_file_path(&header) {
                            let content = lines[content_start..end].join("\n");
                            sections.push((header, content));
                        }
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

fn clean_header_path(line: &str) -> String {
    line.trim()
        .trim_start_matches('#')
        .trim()
        .trim_matches('*')
        .trim_matches('`')
        .trim()
        .to_string()
}

fn looks_like_file_path(header: &str) -> bool {
    if header.is_empty() || header.ends_with(':') {
        return false;
    }
    if header.contains(' ') && !header.contains('/') && !header.contains('\\') {
        return false;
    }
    true
}

fn detect_fence(line: &str) -> Option<char> {
    let trimmed = line.trim();
    if trimmed.starts_with(BACKTICK_FENCE) {
        Some('`')
    } else if trimmed.starts_with(TILDE_FENCE) {
        Some('~')
    } else {
        None
    }
}

fn fence_info_string(line: &str) -> &str {
    let trimmed = line.trim();
    let without_fence = trimmed.trim_start_matches('`').trim_start_matches('~');
    without_fence.trim()
}

pub fn normalize_relpath(p: &str) -> String {
    let mut p = p
        .trim()
        .trim_matches(|c| c == '\'' || c == '"' || c == '`')
        .replace('\\', "/");
    while p.starts_with('/') || p.starts_with("./") {
        if p.starts_with("./") {
            p = p[2..].to_string();
        } else {
            p = p[1..].to_string();
        }
    }
    if let Some(normalized) = normalize_path(&p) {
        p = normalized;
    }
    p
}

fn normalize_path(p: &str) -> Option<String> {
    let mut parts = Vec::new();
    for component in p.split('/') {
        if component == ".." {
            parts.pop();
        } else if component != "." && !component.is_empty() {
            parts.push(component);
        }
    }
    if parts.is_empty() {
        Some(".".to_string())
    } else {
        Some(parts.join("/"))
    }
}