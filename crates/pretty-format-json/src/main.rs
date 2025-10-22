use anyhow::Result;
use clap::Parser;
use precommit_rs::{cli, RunContext};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Pretty-format JSON files (in-place).",
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

    /// Files or directories to format
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let Cli {
        dry_run,
        debug,
        paths,
    } = Cli::parse();

    let mut ctx = RunContext::default();
    ctx.dry_run = dry_run;
    ctx.debug = debug;

    precommit_rs::hooks::pretty_format_json::run_with_ctx(&ctx, paths)
}
