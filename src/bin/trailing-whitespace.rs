use precommit_rs::RunContext;
fn main() -> anyhow::Result<()> {
    let ctx = RunContext::default();
    let args: Vec<std::path::PathBuf> = std::env::args().skip(1).map(|s| s.into()).collect();
    precommit_rs::hooks::trailing_whitespace::run_with_ctx(&ctx, args)
}
