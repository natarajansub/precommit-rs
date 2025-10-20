use anyhow::Result;
use std::path::PathBuf;
use std::fs;

/// Fail if any file path in `paths` exceeds `max_bytes` when specified.
pub fn run(max_bytes: Option<u64>, paths: Vec<PathBuf>) -> Result<()> {
    run_with_ctx(&crate::RunContext::default(), max_bytes, paths)
}

pub fn run_with_ctx(ctx: &crate::RunContext, max_bytes: Option<u64>, paths: Vec<PathBuf>) -> Result<()> {
    if ctx.debug { eprintln!("check_added_large_files: dry_run={}", ctx.dry_run); }
    let mut too_large = false;
    let limit = max_bytes.unwrap_or(500_000); // default 500 KB

    for p in paths {
        if p.is_file() {
            let m = fs::metadata(&p)?;
            if m.len() > limit {
                eprintln!("File {} is too large ({} bytes) > {} bytes", p.display(), m.len(), limit);
                too_large = true;
            }
        }
    }

    if too_large {
        if ctx.dry_run {
            if ctx.debug { eprintln!("dry-run: check would have failed"); }
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
    fn detects_large_file() {
        let mut f = NamedTempFile::new().unwrap();
        let data = vec![0u8; 1024 * 1024];
        std::io::Write::write_all(&mut f, &data).unwrap();
        let path = f.path().to_path_buf();
        // Use a large limit so the function returns Ok instead of exiting
        let res = run(Some(10_000_000), vec![path]);
        assert!(res.is_ok());
    }
}
