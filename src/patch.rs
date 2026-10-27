use crate::error::{Error, Result};
use crate::split::{is_patch_block, normalize_relpath, parse_sections};
use clap::Args;
use regex::Regex;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

const SEARCH_MARKER_4: &str = concat!("<<<<", " SEARCH");
const SEARCH_MARKER_3: &str = concat!("<<<", " SEARCH");
const REPLACE_MARKER_4: &str = concat!(">>>>", " REPLACE");
const REPLACE_MARKER_3: &str = concat!(">>>", " REPLACE");

#[derive(Args, Debug)]
#[command(
    name = "patch",
    about = "Applies search/replace or unified diff code patches from piped input or a file."
)]
pub struct PatchArgs {
    /// Optional path to the input file via flag.
    #[arg(short, long)]
    pub input: Option<PathBuf>,

    /// Optional positional path to the input file. If omitted, reads from stdin.
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Output directory root.
    #[arg(short = 'd', long, default_value = ".")]
    pub outdir: PathBuf,

    /// Dry run: show what would be patched without modifying files.
    #[arg(long, help = "Dry run: show what would be patched without modifying files.")]
    pub dry_run: bool,
}

pub fn run(args: PatchArgs) -> Result<()> {
    let input_path = args.input.as_ref().or(args.file.as_ref());
    let content = match input_path {
        Some(path) => {
            if !path.exists() || !path.is_file() {
                return Err(Error::Custom(format!(
                    "error: input_not_found, path={}",
                    path.display()
                )));
            }
            fs::read_to_string(path)?
        }
        None => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            buffer
        }
    };

    execute_patch(&content, &args.outdir, args.dry_run)
}

pub fn execute_patch(content: &str, outdir_arg: &Path, dry_run: bool) -> Result<()> {
    let outdir = outdir_arg
        .canonicalize()
        .unwrap_or_else(|_| outdir_arg.to_path_buf());
    let normalized_content = content.replace("\r\n", "\n");
    let sections = parse_sections(&normalized_content, "fenced");

    let search_replace_re = Regex::new(
        r"(?s)<{3,7}[ \t]*SEARCH[ \t]*\n(.*?)\n={3,7}[ \t]*\n(.*?)(?:\n>{3,7}[ \t]*REPLACE[ \t]*|>{3,7}[ \t]*REPLACE[ \t]*)",
    )?;

    let mut total_applied_hunks = 0;
    let mut failed_hunks = 0;

    for (raw_rel, code_block) in sections {
        if !is_patch_block(&code_block) {
            continue;
        }

        let safe_rel = normalize_relpath(&raw_rel);
        if safe_rel.is_empty() || safe_rel == "." {
            continue;
        }

        let file_path = outdir.join(&safe_rel);
        let file_exists = file_path.exists();

        let mut file_content = if file_exists {
            fs::read_to_string(&file_path)?.replace("\r\n", "\n")
        } else {
            String::new()
        };

        let mut file_patched = false;
        let mut hunks_in_file = 0;

        let has_sr_markers = code_block.lines().any(|l| {
            let t = l.trim();
            t == SEARCH_MARKER_4 || t == SEARCH_MARKER_3
        }) && code_block.lines().any(|l| {
            let t = l.trim();
            t == REPLACE_MARKER_4 || t == REPLACE_MARKER_3
        });

        if has_sr_markers {
            for patch_cap in search_replace_re.captures_iter(&code_block) {
                let search_str = &patch_cap[1];
                let replace_str = &patch_cap[2];

                if !file_exists && search_str.trim().is_empty() {
                    file_content = replace_str.to_string();
                    file_patched = true;
                    hunks_in_file += 1;
                    total_applied_hunks += 1;
                    continue;
                }

                if !file_exists {
                    eprintln!("⚠️ Cannot patch missing file: {}", safe_rel);
                    log::warn!(
                        "warning: file_not_found_for_patching, path={}",
                        file_path.display()
                    );
                    failed_hunks += 1;
                    break;
                }

                if let Some(updated) = apply_search_replace(&file_content, search_str, replace_str)
                {
                    file_content = updated;
                    file_patched = true;
                    hunks_in_file += 1;
                    total_applied_hunks += 1;
                } else {
                    eprintln!(
                        "❌ Search block not found in {}:\n---\n{}\n---",
                        safe_rel,
                        search_str.lines().take(5).collect::<Vec<_>>().join("\n")
                    );
                    log::error!("error: search_block_not_found, path={}", file_path.display());
                    failed_hunks += 1;
                }
            }
        } else if code_block
            .lines()
            .any(|l| l.starts_with("@@ ") && l.contains(" @@"))
        {
            if !file_exists {
                eprintln!("⚠️ Cannot patch missing file: {}", safe_rel);
                failed_hunks += 1;
                continue;
            }
            match apply_unified_hunks(&file_content, &code_block) {
                Ok((updated, count)) if count > 0 => {
                    file_content = updated;
                    file_patched = true;
                    hunks_in_file += count;
                    total_applied_hunks += count;
                }
                Ok(_) => {}
                Err(msg) => {
                    eprintln!("❌ Failed applying unified diff in {}: {}", safe_rel, msg);
                    failed_hunks += 1;
                }
            }
        }

        if file_patched {
            if !file_content.ends_with('\n') {
                file_content.push('\n');
            }
            if dry_run {
                println!(
                    "🔍 [dry-run] Would patch {} ({} hunk(s))",
                    safe_rel, hunks_in_file
                );
                log::info!("plan_patch, path={}", file_path.display());
            } else {
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&file_path, file_content)?;
                println!("✅ Patched {} ({} hunk(s))", safe_rel, hunks_in_file);
                log::info!("patched_file, path={}", file_path.display());
            }
        }
    }

    log::info!(
        "summary, patches={}, failed={}, dry_run={}, outdir={}",
        total_applied_hunks,
        failed_hunks,
        dry_run,
        outdir.display()
    );

    if failed_hunks > 0 {
        return Err(Error::Custom(format!(
            "error: {} patch hunk(s) failed to apply",
            failed_hunks
        )));
    }

    Ok(())
}

fn apply_search_replace(
    file_content: &str,
    search_str: &str,
    replace_str: &str,
) -> Option<String> {
    if !search_str.is_empty() && file_content.contains(search_str) {
        return Some(file_content.replacen(search_str, replace_str, 1));
    }

    let file_lines: Vec<&str> = file_content.lines().collect();
    let search_lines: Vec<&str> = search_str.lines().collect();

    if search_lines.is_empty() || search_lines.len() > file_lines.len() {
        return None;
    }

    for start_idx in 0..=(file_lines.len() - search_lines.len()) {
        let matches = search_lines
            .iter()
            .enumerate()
            .all(|(offset, s_line)| file_lines[start_idx + offset].trim_end() == s_line.trim_end());

        if matches {
            let mut result_lines: Vec<&str> = Vec::new();
            result_lines.extend_from_slice(&file_lines[..start_idx]);
            if !replace_str.is_empty() {
                result_lines.extend(replace_str.lines());
            }
            result_lines.extend_from_slice(&file_lines[(start_idx + search_lines.len())..]);
            return Some(result_lines.join("\n"));
        }
    }

    None
}

fn apply_unified_hunks(
    file_content: &str,
    diff_block: &str,
) -> std::result::Result<(String, usize), String> {
    let mut current_content = file_content.to_string();
    let mut applied = 0;
    let mut in_hunk = false;
    let mut search_lines: Vec<String> = Vec::new();
    let mut replace_lines: Vec<String> = Vec::new();

    let flush_hunk = |content: &mut String,
                      s_lines: &mut Vec<String>,
                      r_lines: &mut Vec<String>,
                      count: &mut usize|
     -> std::result::Result<(), String> {
        if s_lines.is_empty() && r_lines.is_empty() {
            return Ok(());
        }
        let search_block = s_lines.join("\n");
        let replace_block = r_lines.join("\n");
        s_lines.clear();
        r_lines.clear();

        if let Some(updated) = apply_search_replace(content, &search_block, &replace_block) {
            *content = updated;
            *count += 1;
            Ok(())
        } else {
            Err(format!(
                "Hunk context mismatch near:\n{}",
                search_block
                    .lines()
                    .take(4)
                    .collect::<Vec<_>>()
                    .join("\n")
            ))
        }
    };

    for line in diff_block.lines() {
        if line.starts_with("--- ") || line.starts_with("+++ ") {
            continue;
        }
        if line.starts_with("@@ ") && line.contains(" @@") {
            if in_hunk {
                flush_hunk(
                    &mut current_content,
                    &mut search_lines,
                    &mut replace_lines,
                    &mut applied,
                )?;
            }
            in_hunk = true;
            continue;
        }
        if in_hunk {
            if let Some(stripped) = line.strip_prefix('-') {
                search_lines.push(stripped.to_string());
            } else if let Some(stripped) = line.strip_prefix('+') {
                replace_lines.push(stripped.to_string());
            } else if let Some(stripped) = line.strip_prefix(' ') {
                search_lines.push(stripped.to_string());
                replace_lines.push(stripped.to_string());
            } else {
                search_lines.push(line.to_string());
                replace_lines.push(line.to_string());
            }
        }
    }

    if in_hunk {
        flush_hunk(
            &mut current_content,
            &mut search_lines,
            &mut replace_lines,
            &mut applied,
        )?;
    }

    Ok((current_content, applied))
}