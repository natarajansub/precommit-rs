use crate::RunContext;
use anyhow::{anyhow, Context, Result};
use glob::Pattern;
use ignore::WalkBuilder;
use serde::Deserialize;
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const INSTALL_PLACEHOLDER: &str = "{install}";
const TOOLS_DIR: &str = ".precommit-tools";

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
    install: Option<InstallConfig>,
}

#[derive(Debug, Deserialize)]
pub struct InstallConfig {
    #[serde(default)]
    repo: Option<String>,
    #[serde(default)]
    package: Option<String>,
    #[serde(default)]
    entry: Option<String>,
    #[serde(default)]
    binary: Option<String>,
    #[serde(default)]
    language: InstallLanguage,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
    #[serde(default)]
    install_args: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallLanguage {
    Rust,
    Python,
    Node,
}

impl Default for InstallLanguage {
    fn default() -> Self {
        InstallLanguage::Rust
    }
}

impl PreCommitConfig {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let cfg: PreCommitConfig = serde_yaml::from_str(&content)?;
        Ok(cfg)
    }

    pub fn hooks(&self) -> &[HookConfig] {
        self.hooks.as_deref().unwrap_or(&[])
    }
}

// Helper function to collect matching files
fn collect_files(pattern: Option<&String>) -> Result<Vec<PathBuf>> {
    if let Some(pattern) = pattern {
        let pattern = Pattern::new(pattern)
            .map_err(|e| anyhow!("Invalid glob pattern '{}': {}", pattern, e))?;

        let mut paths = Vec::new();
        let walker = WalkBuilder::new(".")
            .standard_filters(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        for entry in walker {
            let entry = entry.map_err(|e| anyhow!("Failed to walk project files: {}", e))?;
            if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                continue;
            }
            if let Some(s) = entry.path().to_str() {
                if pattern.matches(s) {
                    paths.push(entry.path().to_path_buf());
                }
            }
        }
        Ok(paths)
    } else {
        Ok(vec![PathBuf::from(".")])
    }
}

impl HookConfig {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

    pub fn is_builtin(&self) -> bool {
        matches!(
            self.id.as_str(),
            "trailing-whitespace"
                | "end-of-file-fixer"
                | "check-yaml"
                | "pretty-format-json"
                | "check-added-large-files"
        )
    }

    pub fn command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    pub fn command_is_install(&self) -> bool {
        matches!(self.command.as_deref(), Some(INSTALL_PLACEHOLDER))
    }

    pub fn install(&self) -> Option<&InstallConfig> {
        self.install.as_ref()
    }

    pub fn args(&self) -> Option<&[String]> {
        self.args.as_deref()
    }

    pub fn files(&self) -> Option<&str> {
        self.files.as_deref()
    }
}

impl InstallConfig {
    pub fn repo(&self) -> Option<&str> {
        self.repo.as_deref()
    }

    pub fn package(&self) -> Option<&str> {
        self.package.as_deref()
    }

    pub fn entry<'a>(&'a self, hook_id: &'a str) -> &'a str {
        self.entry
            .as_deref()
            .unwrap_or_else(|| self.binary.as_deref().unwrap_or(hook_id))
    }

    pub fn binary<'a>(&'a self, hook_id: &'a str) -> &'a str {
        self.binary
            .as_deref()
            .unwrap_or_else(|| self.entry.as_deref().unwrap_or(hook_id))
    }

    pub fn language(&self) -> InstallLanguage {
        self.language
    }

    pub fn env(&self) -> Option<&HashMap<String, String>> {
        self.env.as_ref()
    }

    pub fn install_args(&self) -> Option<&[String]> {
        self.install_args.as_deref()
    }

    pub fn summary(&self) -> String {
        let target = self
            .package
            .as_deref()
            .or(self.repo.as_deref())
            .unwrap_or("unknown");
        let entry = self.entry.as_deref().unwrap_or("default");
        format!(
            "lang={} target={} entry={}",
            self.language.as_str(),
            target,
            entry
        )
    }
}

impl InstallLanguage {
    fn as_str(self) -> &'static str {
        match self {
            InstallLanguage::Rust => "rust",
            InstallLanguage::Python => "python",
            InstallLanguage::Node => "node",
        }
    }
}

// Helper function to run external commands
fn run_external_command(
    ctx: &RunContext,
    h: &HookConfig,
    cmd: &Path,
    paths: &[PathBuf],
) -> Result<()> {
    if ctx.debug {
        eprintln!("Running external command for {}: {}", h.id, cmd.display());
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
    if let Some(env) = h.install.as_ref().and_then(|i| i.env()) {
        command.envs(env.iter().map(|(k, v)| (k, v)));
    }

    if ctx.debug {
        eprintln!("Running command: {:?}", command);
    }

    let status = command.status().map_err(|e| {
        anyhow!(
            "Failed to execute external command '{}': {}",
            cmd.display(),
            e
        )
    })?;

    if !status.success() {
        return Err(anyhow!(
            "External command '{}' failed with status: {}",
            cmd.display(),
            status
        ));
    }

    Ok(())
}

// Main function to run the hooks from config
pub fn run_config(ctx: &RunContext, cfg: &PreCommitConfig) -> Result<()> {
    let hooks = cfg
        .hooks
        .as_ref()
        .ok_or_else(|| anyhow!("No hooks configured"))?;

    for h in hooks {
        let enabled = h.enabled.unwrap_or(true);
        if !enabled {
            continue;
        }

        // Build list of matching files
        let paths = collect_files(h.files.as_ref())?;

        // Record files being checked in changelog
        for path in &paths {
            ctx.changelog
                .lock()
                .unwrap()
                .record_file_checked(&h.id, path);
        }

        if let Some(cmd) = h.command() {
            let exec_path = if h.command_is_install() {
                if ctx.debug {
                    eprintln!("Ensuring hook '{}' is installed before execution", h.id);
                }
                ensure_installed(ctx, h)?
            } else {
                PathBuf::from(cmd)
            };

            if ctx.debug {
                eprintln!(
                    "Recording change in changelog (external command {} -> {})",
                    h.id,
                    exec_path.display()
                );
            }
            ctx.changelog.lock().unwrap().record_change(
                &h.id,
                &format!("Ran external command: {}", exec_path.display()),
            );
            run_external_command(ctx, h, &exec_path, &paths)?;
        } else {
            // Handle built-in hooks
            match h.id.as_str() {
                "trailing-whitespace" => {
                    if ctx.debug {
                        eprintln!("Running trailing-whitespace from config");
                    }
                    crate::hooks::trailing_whitespace::run_with_ctx(ctx, paths)?;
                }
                "end-of-file-fixer" => {
                    if ctx.debug {
                        eprintln!("Running end-of-file-fixer from config");
                    }
                    crate::hooks::end_of_file::run_with_ctx(ctx, paths)?;
                }
                "check-yaml" => {
                    if ctx.debug {
                        eprintln!("Running check-yaml from config");
                    }
                    crate::hooks::check_yaml::run_with_ctx(ctx, paths)?;
                }
                "pretty-format-json" => {
                    if ctx.debug {
                        eprintln!("Running pretty-format-json from config");
                    }
                    crate::hooks::pretty_format_json::run_with_ctx(ctx, paths)?;
                }
                "check-added-large-files" => {
                    if ctx.debug {
                        eprintln!("Running check-added-large-files from config");
                    }
                    let max_bytes = if let Some(args) = &h.args {
                        args.get(0).and_then(|s| s.parse::<u64>().ok())
                    } else {
                        None
                    };
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

fn ensure_installed(ctx: &RunContext, hook: &HookConfig) -> Result<PathBuf> {
    let install = hook.install().with_context(|| {
        format!(
            "Hook '{}' requires install but no install configuration provided",
            hook.id
        )
    })?;

    let root = env::current_dir()?.join(TOOLS_DIR).join(&hook.id);
    fs::create_dir_all(&root)?;

    let path = match install.language() {
        InstallLanguage::Rust => install_rust(ctx, hook, install, &root)?,
        InstallLanguage::Python => install_python(ctx, hook, install, &root)?,
        InstallLanguage::Node => install_node(ctx, hook, install, &root)?,
    };

    if !path.exists() {
        anyhow::bail!(
            "Expected executable for hook '{}' at {} but it does not exist",
            hook.id,
            path.display()
        );
    }

    Ok(path)
}

fn install_rust(
    ctx: &RunContext,
    hook: &HookConfig,
    install: &InstallConfig,
    root: &Path,
) -> Result<PathBuf> {
    let bin_name = install.binary(hook.id());
    let bin_path = root.join("bin").join(bin_name);
    if bin_path.exists() {
        return Ok(bin_path);
    }

    let target = install.repo().or(install.package()).ok_or_else(|| {
        anyhow!(
            "Install for hook '{}' requires 'repo' or 'package'",
            hook.id
        )
    })?;

    if ctx.debug {
        eprintln!(
            "Installing rust hook '{}' (target {}) into {}",
            hook.id,
            target,
            root.display()
        );
    }

    let mut cmd = Command::new("cargo");
    cmd.arg("install").arg("--force").arg("--root").arg(root);

    if let Some(repo) = install.repo() {
        cmd.arg("--git").arg(repo);
    }

    if let Some(bin) = install.binary.as_ref() {
        cmd.arg("--bin").arg(bin);
    }

    if let Some(args) = install.install_args() {
        cmd.args(args);
    }

    if let Some(package) = install.package() {
        cmd.arg(package);
    }

    run_and_check(cmd, ctx, "cargo install")?;

    Ok(bin_path)
}

fn install_python(
    ctx: &RunContext,
    hook: &HookConfig,
    install: &InstallConfig,
    root: &Path,
) -> Result<PathBuf> {
    let entry = install.entry(hook.id());
    let venv_dir = root.join("venv");
    let bin_dir = python_bin_dir(&venv_dir);
    let executable = bin_dir.join(entry);
    if executable.exists() {
        return Ok(executable);
    }

    fs::create_dir_all(root)?;

    // Install or update the virtual environment using `uv`
    let mut uv = Command::new("uv");
    uv.arg("venv").arg(&venv_dir);
    run_and_check(uv, ctx, "uv venv")?;

    // Determine the package reference (package name or git repo)
    let target = install
        .package()
        .map(|s| s.to_string())
        .or_else(|| install.repo().map(|r| format!("git+{}", r)))
        .ok_or_else(|| {
            anyhow!(
                "Install for hook '{}' requires 'repo' or 'package'",
                hook.id
            )
        })?;

    let python_path = bin_dir.join(if cfg!(windows) {
        "python.exe"
    } else {
        "python"
    });

    let mut uv_pip = Command::new("uv");
    uv_pip
        .arg("pip")
        .arg("install")
        .arg("--python")
        .arg(&python_path)
        .arg("--no-cache");
    if let Some(args) = install.install_args() {
        uv_pip.args(args);
    }
    uv_pip.arg(target);
    run_and_check(uv_pip, ctx, "uv pip install")?;

    Ok(executable)
}

fn install_node(
    ctx: &RunContext,
    hook: &HookConfig,
    install: &InstallConfig,
    root: &Path,
) -> Result<PathBuf> {
    let entry = install.entry(hook.id());
    let bin_path = root.join("node_modules").join(".bin").join(entry);
    if bin_path.exists() {
        return Ok(bin_path);
    }

    fs::create_dir_all(root)?;

    let target = install.package().or(install.repo()).ok_or_else(|| {
        anyhow!(
            "Install for hook '{}' requires 'repo' or 'package'",
            hook.id
        )
    })?;

    if ctx.debug {
        eprintln!(
            "Installing node hook '{}' (target {}) into {}",
            hook.id,
            target,
            root.display()
        );
    }

    let npm = env::var("NPM").unwrap_or_else(|_| "npm".into());
    let mut cmd = Command::new(npm);
    cmd.arg("install").arg("--prefix").arg(root);

    if let Some(args) = install.install_args() {
        cmd.args(args);
    }

    cmd.arg(target);

    run_and_check(cmd, ctx, "npm install")?;

    Ok(bin_path)
}

fn python_bin_dir(venv: &Path) -> PathBuf {
    if cfg!(windows) {
        venv.join("Scripts")
    } else {
        venv.join("bin")
    }
}

fn run_and_check(mut cmd: Command, ctx: &RunContext, label: &str) -> Result<()> {
    if ctx.debug {
        eprintln!("Running {} command: {:?}", label, cmd);
    }
    let status = cmd
        .status()
        .with_context(|| format!("Failed to execute {}", label))?;
    if !status.success() {
        anyhow::bail!("{} command failed with status {}", label, status);
    }
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
        "# For external tools, precommit-rs manages installation automatically.",
        "# Python hooks use the `uv` CLI (https://docs.astral.sh/uv/) to create per-hook virtual environments.",
        "# Ensure `uv` is available on PATH before running these hooks.",
        "# Built-in hooks provided by precommit-rs:",
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
        "  - id: pretty-format-json",
        "    files: '**/*.{json,jsonc}'",
        "    enabled: false",
        "  - id: check-added-large-files",
        "    files: '**/*'",
        "    enabled: false",
        "    args: ['500000']  # optional max size in bytes",
        "",
        "  # Example: install and run a Python tool (managed with `uv venv`)",
        "  - id: ruff-check",
        "    files: '**/*.py'",
        "    enabled: false",
        "    command: \"{install}\"",
        "    install:",
        "      language: python",
        "      package: ruff",
        "      entry: ruff",
        "    args: ['check', '--fix']",
        "",
        "  # Example: use a Node package from npm",
        "  - id: prettier",
        "    files: '**/*.{js,ts,jsx,tsx,json,css,md}'",
        "    enabled: false",
        "    command: \"{install}\"",
        "    install:",
        "      language: node",
        "      package: prettier",
        "      entry: prettier",
        "    args: ['--write']",
        "",
        "  # Example: install a Rust crate from crates.io or Git",
        "  - id: cargo-deny",
        "    files: '**/Cargo.lock'",
        "    enabled: false",
        "    command: \"{install}\"",
        "    install:",
        "      language: rust",
        "      package: cargo-deny",
        "      binary: cargo-deny",
        "    args: ['check']",
        "",
        "  # Example: run a locally available command/binary",
        "  - id: gofmt",
        "    files: '**/*.go'",
        "    enabled: false",
        "    command: gofmt",
        "    args: ['-w']",
    ];
    let mut sample = lines.join("\n");
    sample.push('\n');
    std::fs::write(path, sample)?;
    Ok(())
}
