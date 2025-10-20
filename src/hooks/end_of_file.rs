use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub fn run(paths: Vec<PathBuf>) -> Result<()> {
    run_with_ctx(&crate::RunContext::default(), paths)
}

pub fn run_with_ctx(ctx: &crate::RunContext, paths: Vec<PathBuf>) -> Result<()> {
    if ctx.debug { eprintln!("end_of_file: dry_run={}", ctx.dry_run); }
    let mut any_changes = false;
    for path in paths {
        if path.is_dir() {
            for entry in walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                let p = entry.path().to_path_buf();
                if p.is_file() {
                    if fix_file_with_ctx(ctx, &p)? {
                        any_changes = true;
                    }
                }
            }
        } else if path.is_file() {
            if fix_file_with_ctx(ctx, &path)? {
                any_changes = true;
            }
        }
    }

    if any_changes {
        if ctx.dry_run {
            if ctx.debug { eprintln!("dry-run: end_of_file would change files"); }
            return Ok(());
        }
        std::process::exit(1);
    }

    Ok(())
}

fn fix_file_with_ctx(ctx: &crate::RunContext, path: &PathBuf) -> Result<bool> {
    // Record this file in changelog as being checked
    ctx.changelog.lock().unwrap().record_file_checked("end-of-file-fixer", path);

    let content = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::InvalidData {
                if ctx.debug { eprintln!("skipping non-utf8 file {}", path.display()); }
                ctx.changelog.lock().unwrap().record_change(
                    "end-of-file-fixer",
                    &format!("Skipped non-UTF8 file: {}", path.display())
                );
                return Ok(false);
            } else {
                return Err(e.into());
            }
        }
    };
    // Remove any trailing newlines then add exactly one
    let trimmed = content.trim_end_matches(|c| c == '\n' || c == '\r');
    let new = format!("{}\n", trimmed);
    if new != content {
        if ctx.dry_run {
            if ctx.debug { eprintln!("dry-run: would fix EOF in {}", path.display()); }
            ctx.changelog.lock().unwrap().record_change(
                "end-of-file-fixer",
                &format!("Would normalize newlines at end of {}", path.display())
            );
            return Ok(true);
        }
        let mut f = fs::OpenOptions::new().write(true).truncate(true).open(path)?;
        f.write_all(new.as_bytes())?;
        ctx.changelog.lock().unwrap().record_change(
            "end-of-file-fixer",
            &format!("Normalized newlines at end of {}", path.display())
        );
        ctx.changelog.lock().unwrap().record_file_modified("end-of-file-fixer", path);
        Ok(true)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn ensures_single_newline() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("b.txt");
        std::fs::write(&file, "x\n\n\n").unwrap();
        let mut ctx = crate::RunContext::default();
        let changed = fix_file_with_ctx(&ctx, &file).unwrap();
        assert!(changed);
        let new = std::fs::read_to_string(&file).unwrap();
        assert_eq!(new, "x\n");
    }
}
