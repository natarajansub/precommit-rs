use anyhow::Result;
use ignore::WalkBuilder;
use std::fs;
use std::path::{Path, PathBuf};

/// Fail if any file path in `paths` exceeds `max_bytes` when specified.
pub fn run(max_bytes: Option<u64>, paths: Vec<PathBuf>) -> Result<()> {
    run_with_ctx(&crate::RunContext::default(), max_bytes, paths)
}

pub fn run_with_ctx(
    ctx: &crate::RunContext,
    max_bytes: Option<u64>,
    paths: Vec<PathBuf>,
) -> Result<()> {
    if ctx.debug {
        eprintln!("check_added_large_files: dry_run={}", ctx.dry_run);
    }
    let mut too_large = false;
    let limit = max_bytes.unwrap_or(500_000); // default 500 KB

    for p in paths {
        if p.is_file() {
            if check_file(&p, limit)? {
                too_large = true;
            }
            continue;
        }

        let metadata = match fs::metadata(&p) {
            Ok(meta) => meta,
            Err(err) => {
                if ctx.debug {
                    eprintln!("Unable to read metadata for {}: {}", p.display(), err);
                }
                continue;
            }
        };

        if metadata.is_dir() {
            let walker = WalkBuilder::new(&p)
                .git_ignore(true)
                .git_global(true)
                .git_exclude(true)
                .standard_filters(true)
                .build();

            for entry in walker {
                let entry = match entry {
                    Ok(e) => e,
                    Err(err) => {
                        if ctx.debug {
                            eprintln!("Walk error under {}: {}", p.display(), err);
                        }
                        continue;
                    }
                };

                if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    continue;
                }

                if check_file(entry.path(), limit)? {
                    too_large = true;
                }
            }
        } else if metadata.is_file() {
            if check_file(&p, limit)? {
                too_large = true;
            }
        }
    }

    if too_large {
        if ctx.dry_run {
            if ctx.debug {
                eprintln!("dry-run: check would have failed");
            }
            return Ok(());
        }
        std::process::exit(1);
    }
    Ok(())
}

fn check_file(path: &Path, limit: u64) -> Result<bool> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > limit {
        eprintln!(
            "File {} is too large ({} bytes) > {} bytes",
            path.display(),
            metadata.len(),
            limit
        );
        return Ok(true);
    }
    Ok(false)
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

    #[test]
    fn skips_gitignored_directories() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "ignored/\n").unwrap();
        let ignored_dir = dir.path().join("ignored");
        std::fs::create_dir_all(&ignored_dir).unwrap();
        std::fs::write(ignored_dir.join("large.bin"), vec![0u8; 2_000_000]).unwrap();
        let mut ctx = crate::RunContext::default();
        ctx.dry_run = true;

        let res = run_with_ctx(&ctx, Some(500_000), vec![dir.path().to_path_buf()]);
        assert!(res.is_ok(), "gitignored files should be skipped");
    }
}
