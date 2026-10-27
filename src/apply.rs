use crate::error::{Error, Result};
use crate::patch;
use crate::split::{self, is_patch_block, parse_sections};
use clap::Args;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

#[derive(Args, Debug)]
#[command(
    name = "apply",
    about = "Automatically detects and applies both search/replace patches and full file splits in one pass."
)]
pub struct ApplyArgs {
    /// Path to source text file (.txt/.md/any) via flag.
    #[arg(short, long)]
    pub input: Option<PathBuf>,

    /// Optional positional path to source text file. If omitted, reads from stdin.
    #[arg(value_name = "FILE")]
    pub file: Option<PathBuf>,

    /// Output directory root.
    #[arg(short = 'd', long, default_value = ".")]
    pub outdir: PathBuf,

    /// Parsing mode (used when splitting full files).
    #[arg(long, default_value = "fenced", value_parser = ["fenced", "loose"])]
    pub mode: String,

    /// Allow overwriting existing files when full file blocks are present.
    #[arg(short = 'F', long)]
    pub overwrite: bool,

    /// Show actions without writing files.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(args: ApplyArgs) -> Result<()> {
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

    let normalized = text.replace("\r\n", "\n");
    let sections = parse_sections(&normalized, &args.mode);

    if sections.is_empty() {
        return Err(Error::Custom(
            "error: no_valid_file_sections_found. ensure_filename_precedes_code_block".to_string(),
        ));
    }

    let has_full_files = sections.iter().any(|(_, content)| !is_patch_block(content));
    let has_patches = sections.iter().any(|(_, content)| is_patch_block(content));

    if has_full_files {
        split::execute_split(
            &normalized,
            &args.outdir,
            &args.mode,
            args.overwrite,
            args.dry_run,
            true,
        )?;
    }

    if has_patches {
        patch::execute_patch(&normalized, &args.outdir, args.dry_run)?;
    }

    Ok(())
}