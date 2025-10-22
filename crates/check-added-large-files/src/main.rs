use anyhow::Result;
use clap::Parser;
use precommit_rs::{cli, RunContext};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Fail if added files exceed a size limit (in bytes).",
    color = clap::ColorChoice::Always,
    styles = cli::styles()
)]
struct Cli {
    /// Do not write changes, only report what would be changed
    #[arg(long)]
    dry_run: bool,

    /// Enable debug output
    #[arg(long)]
    debug: bool,

    /// Maximum allowed file size in bytes
    #[arg(long, value_name = "BYTES")]
    max_bytes: Option<u64>,

    /// Files or directories to check
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let Cli {
        dry_run,
        debug,
        max_bytes,
        paths,
    } = Cli::parse();

    let mut ctx = RunContext::default();
    ctx.dry_run = dry_run;
    ctx.debug = debug;

    precommit_rs::hooks::check_added_large_files::run_with_ctx(&ctx, max_bytes, paths)
}
