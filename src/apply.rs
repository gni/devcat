use crate::error::Result;
use crate::{patch, split};
use clap::Args;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

#[derive(Args, Debug)]
#[command(name = "apply", about = "Automatically detects and applies either a patch or a full file split.")]
pub struct ApplyArgs {
    /// path to source text file (.txt/.md/any). If omitted, reads from stdin.
    #[arg(short, long)]
    pub input: Option<PathBuf>,

    /// output directory root (used if delegating to split)
    #[arg(short = 'd', long, default_value = ".")]
    pub outdir: PathBuf,

    /// parsing mode (used if delegating to split)
    #[arg(long, default_value = "fenced", value_parser = ["fenced", "loose"])]
    pub mode: String,

    /// allow overwriting existing files (used if delegating to split)
    #[arg(short = 'F', long)]
    pub overwrite: bool,

    /// show actions without writing files
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(args: ApplyArgs) -> Result<()> {
    let text = match &args.input {
        Some(src) => {
            if !src.exists() || !src.is_file() {
                return Err(crate::error::Error::Custom(format!("error: input_not_found, path={}", src.display())).into());
            }
            fs::read_to_string(src)?
        }
        None => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            buffer
        }
    };

    split::execute_split(&text, &args.outdir, &args.mode, args.overwrite, args.dry_run, true)?;

    if text.contains("<<<< SEARCH") && text.contains(">>>> REPLACE") {
        patch::execute_patch(&text, &args.outdir, args.dry_run)?;
    }

    Ok(())
}
