use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

pub fn run(paths: Vec<PathBuf>) -> Result<()> {
    run_with_ctx(&crate::RunContext::default(), paths)
}

pub fn run_with_ctx(ctx: &crate::RunContext, paths: Vec<PathBuf>) -> Result<()> {
    if ctx.debug { eprintln!("trailing_whitespace: dry_run={}", ctx.dry_run); }
    let mut any_changes = false;
    for path in paths {
        if path.is_dir() {
            for entry in walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                let p = entry.path().to_path_buf();
                if p.is_file() {
                    if ctx.debug { eprintln!("processing {}", p.display()); }
                    match fix_file_with_ctx(ctx, &p) {
                        Ok(changed) => if changed { any_changes = true },
                        Err(e) => {
                            if ctx.debug { eprintln!("error processing {}: {}", p.display(), e); continue; }
                            return Err(e);
                        }
                    }
                }
            }
        } else if path.is_file() {
            match fix_file_with_ctx(ctx, &path) {
                Ok(changed) => if changed { any_changes = true },
                Err(e) => {
                    if ctx.debug { eprintln!("error processing {}: {}", path.display(), e); continue; }
                    return Err(e);
                }
            }
        }
    }

    if any_changes {
        if ctx.dry_run {
            if ctx.debug { eprintln!("dry-run: changes would have been made"); }
            return Ok(());
        }
        // pre-commit expects exit code 1 when changes are made
        std::process::exit(1);
    }

    Ok(())
}

fn fix_file_with_ctx(ctx: &crate::RunContext, path: &PathBuf) -> Result<bool> {
    let content = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::InvalidData {
                if ctx.debug { eprintln!("skipping non-utf8 file {}", path.display()); }
                return Ok(false);
            } else {
                return Err(e.into());
            }
        }
    };
    let mut changed = false;
    let mut out = String::with_capacity(content.len());

    for line in content.lines() {
        let trimmed = line.trim_end_matches(|c| c == ' ' || c == '\t');
        if trimmed.len() != line.len() {
            changed = true;
        }
        out.push_str(trimmed);
        out.push('\n');
    }

    if changed {
        if ctx.dry_run {
            if ctx.debug { eprintln!("dry-run: would fix trailing whitespace in {}", path.display()); }
            ctx.changelog.lock().unwrap().record_change(
                "trailing-whitespace",
                &format!("Would remove trailing whitespace from {}", path.display())
            );
            return Ok(true);
        }
        let mut f = fs::OpenOptions::new().write(true).truncate(true).open(path)?;
        f.write_all(out.as_bytes())?;
        ctx.changelog.lock().unwrap().record_change(
            "trailing-whitespace",
            &format!("Removed trailing whitespace from {}", path.display())
        );
        ctx.changelog.lock().unwrap().record_file_modified("trailing-whitespace", path);
    }

    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn removes_trailing_spaces() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "hello \nworld\t \n") .unwrap();
        let ctx = crate::RunContext::default();
        let changed = fix_file_with_ctx(&ctx, &file).unwrap();
        assert!(changed);
        let new = std::fs::read_to_string(&file).unwrap();
        assert_eq!(new, "hello\nworld\n");
    }
}
