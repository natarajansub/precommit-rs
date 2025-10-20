use anyhow::Result;
use std::path::PathBuf;
use std::fs;

pub fn run(paths: Vec<PathBuf>) -> Result<()> {
    run_with_ctx(&crate::RunContext::default(), paths)
}

pub fn run_with_ctx(ctx: &crate::RunContext, paths: Vec<PathBuf>) -> Result<()> {
    if ctx.debug { eprintln!("pretty_format_json: dry_run={}", ctx.dry_run); }
    let mut any_changes = false;
    for p in paths {
        if p.is_file() {
            if format_file_with_ctx(ctx, &p)? {
                any_changes = true;
            }
        }
    }
    if any_changes {
        if ctx.dry_run {
            if ctx.debug { eprintln!("dry-run: pretty_format_json would have changed files"); }
            return Ok(());
        }
        std::process::exit(1);
    }
    Ok(())
}

fn format_file_with_ctx(ctx: &crate::RunContext, path: &PathBuf) -> Result<bool> {
    let content = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::InvalidData {
                if ctx.debug { eprintln!("skipping non-utf8 file {}", path.display()); }
                return Ok(false);
            } else { return Err(e.into()); }
        }
    };
    let v: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Ok(false),
    };
    let new = serde_json::to_string_pretty(&v)? + "\n";
    if new != content {
        if ctx.dry_run {
            if ctx.debug { eprintln!("dry-run: would format JSON in {}", path.display()); }
            ctx.changelog.lock().unwrap().record_change(
                "pretty-format-json",
                &format!("Would format JSON in {}", path.display())
            );
            return Ok(true);
        }
        fs::write(path, new)?;
        ctx.changelog.lock().unwrap().record_change(
            "pretty-format-json",
            &format!("Formatted JSON in {}", path.display())
        );
        ctx.changelog.lock().unwrap().record_file_modified("pretty-format-json", path);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn formats_json() {
        let mut f = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut f, b"{\"a\":1}").unwrap();
        let path = f.path().to_path_buf();
        let ctx = crate::RunContext::default();
        let changed = format_file_with_ctx(&ctx, &path).unwrap();
        assert!(changed);
    }
}
