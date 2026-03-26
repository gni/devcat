use crate::error::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug)]
#[command(name = "split")]
pub struct SplitArgs {
    /// path to source text file (.txt/.md/any)
    #[arg(short, long)]
    pub input: PathBuf,

    /// output directory root
    #[arg(short = 'd', long)]
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
    let src = &args.input;
    let outdir = args.outdir.canonicalize().unwrap_or_else(|_| args.outdir.clone());

    if !src.exists() || !src.is_file() {
        return Err(crate::error::Error::Custom(format!("error: input_not_found, path={}", src.display())).into());
    }

    std::fs::create_dir_all(&outdir)?;

    let text = std::fs::read_to_string(src)?;
    let sections = parse_sections(&text, &args.mode);

    if sections.is_empty() {
        return Err(crate::error::Error::Custom("error: no_sections_detected, hint=ensure_lines_with_file_paths_precede_fenced_code_blocks".to_string()).into());
    }

    // Check for duplicates
    let mut duplicates: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (rel, _) in &sections {
        *duplicates.entry(rel.clone()).or_insert(0) += 1;
    }
    let dups: Vec<&String> = duplicates.iter().filter(|&(_, &count)| count > 1).map(|(k, _)| k).collect();
    if !dups.is_empty() {
        log::warn!("warning: duplicate_targets, files={:?}", dups);
    }

    let mut written = Vec::new();
    for (rel, content) in sections {
        let safe_rel = normalize_relpath(&rel);
        let dest = outdir.join(&safe_rel).canonicalize().unwrap_or_else(|_| outdir.join(&safe_rel));

        // Security check: ensure path doesn't escape output directory
        if !dest.starts_with(&outdir) {
            return Err(crate::error::Error::Custom(format!("error: security_violation, message=path_escapes_output_directory: {}", safe_rel)).into());
        }

        if args.dry_run {
            log::info!("plan_write, path={}, bytes={}", dest.display(), content.len());
            written.push(dest.to_string_lossy().to_string());
            continue;
        }

        if dest.exists() && !args.overwrite {
            return Err(crate::error::Error::Custom(format!("error: file_exists, message=refusing_to_overwrite: {}", dest.display())).into());
        }

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&dest, content.as_bytes())?;
        log::info!("wrote_file, path={}, bytes={}", dest.display(), content.len());
        written.push(dest.to_string_lossy().to_string());
    }

    log::info!("summary, files={}, dry_run={}, outdir={}", written.len(), args.dry_run, outdir.display());

    Ok(())
}

fn normalize_relpath(p: &str) -> String {
    let mut p = p.trim().to_string();
    if (p.starts_with('\'') || p.starts_with('"')) && (p.ends_with('\'') || p.ends_with('"')) && p.len() >= 2 {
        p = p[1..p.len() - 1].to_string();
    }
    p = p.replace('\\', "/");
    while p.starts_with('/') {
        p = p[1..].to_string();
    }
    if let Some(normalized) = normalize_path(&p) {
        p = normalized;
    }
    p = p.replace('\\', "/");
    p
}

fn normalize_path(p: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
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

fn parse_sections(text: &str, mode: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut sections = Vec::new();
    let mut i = 0;
    let n = lines.len();

    while i < n {
        let line = lines[i];
        if let Some(header) = detect_header(line) {
            let mut j = i + 1;
            // Skip empty lines
            while j < n && lines[j].trim().is_empty() {
                j += 1;
            }

            let fence = if j < n { detect_fence(lines[j]) } else { None };

            if mode == "fenced" && fence.is_none() {
                i += 1;
                continue;
            }

            if let Some(fence_char) = fence {
                // Fenced mode
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
                    let content: String = lines[content_start..end].join("\n");
                    sections.push((header, content));
                    i = end + 1;
                    continue;
                }
            } else {
                // Loose mode
                let mut k = j;
                let mut chunk = Vec::new();

                while k < n {
                    if detect_header(lines[k]).is_some() {
                        break;
                    }
                    chunk.push(lines[k]);
                    k += 1;
                }

                let content = chunk.join("\n").trim_end().to_string();
                sections.push((header, content));
                i = k;
                continue;
            }
        }
        i += 1;
    }

    sections
}

fn detect_header(line: &str) -> Option<String> {
    let s = line.trim();
    if s.starts_with("```") || s.starts_with("~~~") {
        return None;
    }

    let trimmed = s.trim_matches(|c| c == '\'' || c == '"');
    if trimmed.is_empty() {
        return None;
    }

    // Check if it looks like a file path
    if trimmed == "." || trimmed == ".." {
        return None;
    }

    // Simple heuristic: contains a dot or slash, looks like a path
    // Also accept lines that look like imports (e.g., "import argparse")
    if trimmed.contains('.') || trimmed.contains('/') || trimmed.starts_with("import ") || trimmed.starts_with("from ") {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn detect_fence(line: &str) -> Option<char> {
    let trimmed = line.trim();
    if trimmed.starts_with("```") {
        Some('`')
    } else if trimmed.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}
