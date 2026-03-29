use crate::error::Result;
use clap::Args;
use regex::Regex;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[derive(Args, Debug)]
#[command(name = "patch", about = "Applies search/replace code patches from piped input or a file.")]
pub struct PatchArgs {
    #[arg(help = "Optional path to the input file. If not provided, reads from stdin.")]
    pub input: Option<PathBuf>,

    #[arg(long, help = "Dry run: show what would be patched without modifying files.")]
    pub dry_run: bool,
}

pub fn run(args: PatchArgs) -> Result<()> {
    let content = match &args.input {
        Some(path) => fs::read_to_string(path)?,
        None => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            buffer
        }
    };

    execute_patch(&content, Path::new("."), args.dry_run)
}

pub fn execute_patch(content: &str, outdir_arg: &Path, dry_run: bool) -> Result<()> {
    let outdir = outdir_arg.canonicalize().unwrap_or_else(|_| outdir_arg.to_path_buf());
    let normalized_content = content.replace("\r\n", "\n");
    let file_block_re = Regex::new(r"(?:^|\n)([^\n]+)\n```[a-zA-Z0-9_]*\n([\s\S]*?)\n```")?;
    let patch_block_re = Regex::new(r"(?s)<<<< SEARCH[ \t]*\n(.*?)\n====[ \t]*\n(.*?)\n>>>> REPLACE[ \t]*")?;

    let mut applied_patches = 0;

    for file_cap in file_block_re.captures_iter(&normalized_content) {
        let file_path_str = file_cap[1].trim();
        let raw_path = Path::new(file_path_str);
        let code_block = &file_cap[2];

        if file_path_str.is_empty() || file_path_str.contains("..") || raw_path.is_absolute() {
            log::warn!("warning: skipped_invalid_path, path={}", file_path_str);
            continue;
        }

        if !code_block.contains("<<<< SEARCH") || !code_block.contains(">>>> REPLACE") {
            continue;
        }

        let file_path = outdir.join(raw_path);

        if !file_path.exists() {
            log::warn!("warning: file_not_found_for_patching, path={}", file_path.display());
            continue;
        }

        let mut file_content = fs::read_to_string(&file_path)?.replace("\r\n", "\n");
        let mut patched = false;

        for patch_cap in patch_block_re.captures_iter(code_block) {
            let search_str = &patch_cap[1];
            let replace_str = &patch_cap[2];

            if file_content.contains(search_str) {
                file_content = file_content.replace(search_str, replace_str);
                patched = true;
                applied_patches += 1;
            } else {
                log::error!("error: search_block_not_found_check_indentation, path={}", file_path.display());
            }
        }

        if patched {
            if dry_run {
                log::info!("plan_patch, path={}", file_path.display());
            } else {
                fs::write(&file_path, file_content)?;
                log::info!("patched_file, path={}", file_path.display());
            }
        }
    }

    log::info!("summary, patches={}, dry_run={}, outdir={}", applied_patches, dry_run, outdir.display());

    Ok(())
}