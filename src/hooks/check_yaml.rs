use anyhow::Result;
use std::path::PathBuf;
use std::fs;

pub fn run(paths: Vec<PathBuf>) -> Result<()> {
    run_with_ctx(&crate::RunContext::default(), paths)
}

pub fn run_with_ctx(ctx: &crate::RunContext, paths: Vec<PathBuf>) -> Result<()> {
    if ctx.debug { eprintln!("check_yaml: dry_run={}", ctx.dry_run); }
    let mut had_error = false;
    for p in paths {
        if p.is_file() {
            ctx.changelog.lock().unwrap().record_file_checked("check-yaml", &p);

            let content = match fs::read_to_string(&p) {
                Ok(s) => s,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::InvalidData {
                        if ctx.debug { eprintln!("skipping non-utf8 file {}", p.display()); }
                        ctx.changelog.lock().unwrap().record_change(
                            "check-yaml",
                            &format!("Skipped non-UTF8 file: {}", p.display())
                        );
                        continue;
                    } else { return Err(e.into()); }
                }
            };
            if let Err(e) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                eprintln!("YAML parse error in {}: {}", p.display(), e);
                ctx.changelog.lock().unwrap().record_change(
                    "check-yaml",
                    &format!("Invalid YAML in {}: {}", p.display(), e)
                );
                had_error = true;
            }
        }
    }
    if had_error {
        if ctx.dry_run {
            if ctx.debug { eprintln!("dry-run: check-yaml would have failed"); }
            return Ok(());
        }
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn valid_yaml_ok() {
        let mut f = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut f, b"a: 1\nb: [1,2]").unwrap();
        let res = run(vec![f.path().to_path_buf()]);
        assert!(res.is_ok());
    }
}
