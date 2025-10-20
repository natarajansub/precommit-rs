use crate::RunContext;
use anyhow::{anyhow, Result};
use glob::Pattern;
use serde::Deserialize;
use std::{path::{Path, PathBuf}, process::Command};

#[derive(Debug, Deserialize)]
pub struct PreCommitConfig {
    hooks: Option<Vec<HookConfig>>,
}

#[derive(Debug, Deserialize)]
pub struct HookConfig {
    id: String,
    enabled: Option<bool>,
    args: Option<Vec<String>>,
    files: Option<String>,
    // External command to run instead of built-in hook
    command: Option<String>,
    // Working directory for external command
    #[serde(rename = "working-dir")]
    working_dir: Option<String>,
}

impl PreCommitConfig {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let cfg: PreCommitConfig = serde_yaml::from_str(&content)?;
        Ok(cfg)
    }
}

// Helper function to collect matching files
fn collect_files(pattern: Option<&String>) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    if let Some(pattern) = pattern {
        // Build the glob pattern for file matching
        let pattern = Pattern::new(pattern)
            .map_err(|e| anyhow!("Invalid glob pattern '{}': {}", pattern, e))?;

        // Walk directory and filter files
        for entry in walkdir::WalkDir::new(".")
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let p = entry.path();
            // Skip target and .git directories
            if p.components().any(|c| c.as_os_str() == "target" || c.as_os_str() == ".git") {
                continue;
            }
            if p.is_file() {
                if let Some(s) = p.to_str() {
                    if pattern.matches(s) {
                        paths.push(p.to_path_buf());
                    }
                }
            }
        }
    } else {
        // Default: run on current directory but skip target/.git
        paths.push(PathBuf::from("."));
    }
    Ok(paths)
}

// Helper function to run external commands
fn run_external_command(ctx: &RunContext, h: &HookConfig, cmd: &str, paths: &[PathBuf]) -> Result<()> {
    if ctx.debug {
        eprintln!("Running external command for {}: {}", h.id, cmd);
    }

    let mut command = Command::new(cmd);
    
    // Add any configured arguments
    if let Some(args) = &h.args {
        command.args(args);
    }
    
    // Add paths as arguments (common pattern for external tools)
    command.args(paths.iter().map(|p| p.as_os_str()));

    // Change working directory if specified
    if let Some(dir) = &h.working_dir {
        command.current_dir(dir);
    }

    // Run the command
    if ctx.debug {
        eprintln!("Running command: {:?}", command);
    }

    let status = command.status()
        .map_err(|e| anyhow!("Failed to execute external command '{}': {}", cmd, e))?;

    if !status.success() {
        return Err(anyhow!("External command '{}' failed with status: {}", cmd, status));
    }

    Ok(())
}

// Main function to run the hooks from config
pub fn run_config(ctx: &RunContext, cfg: &PreCommitConfig) -> Result<()> {
    let hooks = cfg.hooks.as_ref().ok_or_else(|| anyhow!("No hooks configured"))?;
    
    for h in hooks {
        let enabled = h.enabled.unwrap_or(true);
        if !enabled { continue; }

        // Build list of matching files
        let paths = collect_files(h.files.as_ref())?;

        // Record files being checked in changelog
        for path in &paths {
            ctx.changelog.lock().unwrap().record_file_checked(&h.id, path);
        }

        if let Some(cmd) = &h.command {
            // For external commands, we can't track exact changes
            // but we can record that the command ran
            if ctx.debug { eprintln!("Recording change in changelog"); }
            ctx.changelog.lock().unwrap().record_change(&h.id, &format!(
                "Ran external command: {}", cmd
            ));
            run_external_command(ctx, h, cmd, &paths)?;
        } else {
            // Handle built-in hooks
            match h.id.as_str() {
                "trailing-whitespace" => {
                    if ctx.debug { eprintln!("Running trailing-whitespace from config"); }
                    crate::hooks::trailing_whitespace::run_with_ctx(ctx, paths)?;
                }
                "end-of-file-fixer" => {
                    if ctx.debug { eprintln!("Running end-of-file-fixer from config"); }
                    crate::hooks::end_of_file::run_with_ctx(ctx, paths)?;
                }
                "check-yaml" => {
                    if ctx.debug { eprintln!("Running check-yaml from config"); }
                    crate::hooks::check_yaml::run_with_ctx(ctx, paths)?;
                }
                "pretty-format-json" => {
                    if ctx.debug { eprintln!("Running pretty-format-json from config"); }
                    crate::hooks::pretty_format_json::run_with_ctx(ctx, paths)?;
                }
                "check-added-large-files" => {
                    if ctx.debug { eprintln!("Running check-added-large-files from config"); }
                    let max_bytes = if let Some(args) = &h.args {
                        args.get(0).and_then(|s| s.parse::<u64>().ok())
                    } else { None };
                    crate::hooks::check_added_large_files::run_with_ctx(ctx, max_bytes, paths)?;
                }
                _ => {
                    eprintln!("Unknown hook id in config: {}", h.id);
                }
            }
        }
    }

    // Write changelog if there were any changes
    ctx.changelog.lock().unwrap().write_if_changed()?;

    Ok(())
}

// Write a default config file with examples
pub fn write_default_config(path: &std::path::Path) -> Result<()> {
    let lines = [
        "# .pre-commit.yaml generated by precommit-rs",
        "# Each hook has a 'files' glob pattern that matches files to check",
        "# Globs can use: ? (single char), * (any chars), ** (recursive dirs)",
        "# Example: '*.{rs,toml}' matches Rust files and Cargo.toml",
        "#",
        "# Built-in hooks:",
        "hooks:",
        "  - id: trailing-whitespace",
        "    files: '**/*.{rs,py,js,ts,txt,md}'",
        "    enabled: true",
        "  - id: end-of-file-fixer",
        "    files: '**/*.{rs,py,txt,md}'",
        "    enabled: true",
        "  - id: check-yaml",
        "    files: '**/*.{yml,yaml}'",
        "    enabled: true",
        "",
        "  # Python Hooks:",
        "  - id: ruff-check",
        "    files: '**/*.py'",
        "    enabled: false",
        "    command: ruff",
        "    args: ['check', '--fix']",
        "",
        "  - id: ruff-format",
        "    files: '**/*.py'",
        "    enabled: false",
        "    command: ruff",
        "    args: ['format']",
        "",
        "  - id: black",
        "    files: '**/*.py'",
        "    enabled: false",
        "    command: black",
        "    args: ['--quiet']",
        "",
        "  - id: mypy",
        "    files: '**/*.py'",
        "    enabled: false",
        "    command: mypy",
        "    args: ['--ignore-missing-imports']",
        "",
        "  # TypeScript/JavaScript Hooks:",
        "  - id: eslint",
        "    files: '**/*.{js,ts,jsx,tsx}'",
        "    enabled: false",
        "    command: eslint",
        "    args: ['--fix']",
        "",
        "  - id: prettier",
        "    files: '**/*.{js,ts,jsx,tsx,json,css,md}'",
        "    enabled: false",
        "    command: prettier",
        "    args: ['--write']",
        "",
        "  # Golang Hooks:",
        "  - id: gofmt",
        "    files: '**/*.go'",
        "    enabled: false",
        "    command: gofmt",
        "    args: ['-w']",
        "",
        "  - id: golangci-lint",
        "    files: '**/*.go'",
        "    enabled: false",
        "    command: golangci-lint",
        "    args: ['run', '--fix']",
        "",
        "  # Rust Hooks:",
        "  - id: cargo-fmt",
        "    files: '**/*.rs'",
        "    enabled: false",
        "    command: cargo",
        "    args: ['fmt']",
        "",
        "  - id: cargo-clippy",
        "    files: '**/*.rs'",
        "    enabled: false",
        "    command: cargo",
        "    args: ['clippy', '--fix', '--allow-dirty']",
        "",
        "  - id: typos",
        "    files: '**/*.{rs,py,js,ts,go,md}'",
        "    enabled: false",
        "    command: typos",
        "    args: ['--write-changes']",
    ];
    let mut sample = lines.join("\n");
    sample.push('\n');
    std::fs::write(path, sample)?;
    Ok(())
}