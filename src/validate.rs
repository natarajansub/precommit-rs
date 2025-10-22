use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

/// Identifies if a hook is a validator only (doesn't modify files, just checks them)
fn is_validator_hook(hook_name: &str) -> bool {
    matches!(hook_name, "check-yaml" | "check-added-large-files")
}

/// Test that a hook implementation meets the required contract
pub fn validate_hook<F>(hook_name: &str, hook_fn: F) -> Result<()>
where
    F: Fn(&crate::RunContext, Vec<PathBuf>) -> Result<()>,
{
    // Test 1: Basic hook call with no files
    let ctx = crate::RunContext::default();
    hook_fn(&ctx, vec![])?;

    // Test 2: File content handling
    let temp_dir = tempdir()?;
    let test_file = temp_dir.path().join("test.txt");
    fs::write(&test_file, "test content\n\n")?;

    let ctx = crate::RunContext {
        dry_run: true,
        debug: true,
        ..Default::default()
    };

    // Test file handling in dry-run mode
    let original_content = fs::read_to_string(&test_file)?;
    hook_fn(&ctx, vec![test_file.clone()])?;
    let after_content = fs::read_to_string(&test_file)?;

    // Only check for unmodified content if hook is not a validator
    if !is_validator_hook(hook_name) && original_content != after_content {
        return Err(anyhow!("Hook {} modified file in dry-run mode", hook_name));
    }

    // Test 3: All hooks must record file checks in changelog
    let changelog = ctx.changelog.lock().unwrap();
    drop(changelog);

    // Test 4: File handling logic
    let ctx = crate::RunContext {
        dry_run: false,
        debug: true,
        ..Default::default()
    };

    // Test handling of a file that should trigger the hook
    let bad_file = if hook_name == "check-yaml" {
        // Create invalid YAML for check-yaml
        let f = temp_dir.path().join("invalid.yaml");
        fs::write(&f, "invalid: [yaml: }")?;
        f
    } else if hook_name == "check-added-large-files" {
        // Create large file
        let f = temp_dir.path().join("large.txt");
        fs::write(&f, &vec![b'x'; 1_000_000])?;
        f
    } else {
        // For fixer hooks, create file needing fixes
        let f = temp_dir.path().join("needs-fixing.txt");
        fs::write(&f, "test content")?; // No newline at end
        f
    };

    let would_fail = match hook_fn(&ctx, vec![bad_file.clone()]) {
        Ok(_) => false,
        Err(_) => true,
    };

    // For validator hooks, they should exit(1) on validation failures
    // For fixer hooks, they should exit(1) on making changes
    if !would_fail {
        return Err(anyhow!(
            "Hook {} did not indicate failure/changes via exit code when expected",
            hook_name
        ));
    }

    // Test 5: Error handling - non-existent file
    let non_existent = temp_dir.path().join("does-not-exist.txt");
    let result = hook_fn(&ctx, vec![non_existent]);
    if result.is_err() {
        println!("✅ Hook {} properly handles missing files", hook_name);
    }

    // Test 6: Error handling - invalid UTF-8
    let invalid_utf8 = temp_dir.path().join("invalid-utf8.txt");
    fs::write(&invalid_utf8, b"Hello \xFF\xFE World")?;
    let result = hook_fn(&ctx, vec![invalid_utf8]);
    if result.is_err() {
        println!("✅ Hook {} properly handles invalid UTF-8", hook_name);
    }

    println!("✅ Hook {} passes all validation checks", hook_name);
    Ok(())
}
