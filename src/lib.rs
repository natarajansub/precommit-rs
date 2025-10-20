pub mod hooks;
pub mod config;
pub mod validate;
pub mod changelog;

use std::sync::{Arc, Mutex};
use changelog::Changelog;

#[derive(Debug, Clone)]
pub struct RunContext {
    pub dry_run: bool,
    pub debug: bool,
    pub changelog: Arc<Mutex<Changelog>>,
}

impl Default for RunContext {
    fn default() -> Self {
        Self {
            dry_run: false,
            debug: false,
            changelog: Arc::new(Mutex::new(Changelog::new())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_changelog_recording() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "test\n\n\n").unwrap();

        // Create RunContext with debug enabled
        let ctx = RunContext {
            debug: true,
            dry_run: true, // Use dry_run mode to prevent exit(1)
            ..Default::default()
        };

        // Run the end-of-file-fixer hook
        let paths = vec![file.clone()];
        let result = hooks::end_of_file::run_with_ctx(&ctx, paths);
        assert!(result.is_ok());

        // Verify changelog contains the changes
        let changelog = ctx.changelog.lock().unwrap();
        assert!(changelog.has_changes());
    }
}
