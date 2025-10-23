use crate::{lock, RunContext};
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
    repos: Option<Vec<RepoConfig>>,
}

#[derive(Debug, Deserialize)]
pub struct RepoConfig {
    repo: String,
    rev: Option<String>,
    #[serde(default)]
    hooks: Vec<HookConfig>,
}

#[derive(Debug, Deserialize)]
pub struct HookConfig {
    id: String,
    name: Option<String>,
    entry: Option<String>,
    language: Option<String>,
    stages: Option<Vec<String>>,
    additional_dependencies: Option<Vec<String>>,
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
    version: Option<String>,
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
    Go,
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

    pub fn repos(&self) -> &[RepoConfig] {
        self.repos.as_deref().unwrap_or(&[])
    }

    pub fn local_hooks(&self) -> Vec<(&RepoConfig, &HookConfig)> {
        self.repos
            .as_ref()
            .map(|repos| {
                repos
                    .iter()
                    .filter(|repo| repo.repo == "local")
                    .flat_map(|repo| repo.hooks.iter().map(move |hook| (repo, hook)))
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl RepoConfig {
    pub fn repo(&self) -> &str {
        &self.repo
    }

    pub fn rev(&self) -> Option<&str> {
        self.rev.as_deref()
    }

    pub fn hooks(&self) -> &[HookConfig] {
        &self.hooks
    }
}

// Helper function to collect matching files
fn expand_pattern(pattern: &str) -> Vec<String> {
    if let (Some(start), Some(end)) = (pattern.find('{'), pattern.find('}')) {
        if end > start {
            let before = &pattern[..start];
            let after = &pattern[end + 1..];
            let inner = &pattern[start + 1..end];
            return inner
                .split(',')
                .map(|alt| format!("{}{}{}", before, alt.trim(), after))
                .collect();
        }
    }
    vec![pattern.to_string()]
}

fn collect_files(pattern: Option<&String>) -> Result<Vec<PathBuf>> {
    if let Some(pattern) = pattern {
        let mut compiled = Vec::new();
        for pat in expand_pattern(pattern) {
            compiled.push(
                Pattern::new(&pat).map_err(|e| anyhow!("Invalid glob pattern '{}': {}", pat, e))?,
            );
        }

        let mut paths = Vec::new();
        let walker = WalkBuilder::new(".")
            .standard_filters(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();
        let root = std::env::current_dir()?;

        for entry in walker {
            let entry = entry.map_err(|e| anyhow!("Failed to walk project files: {}", e))?;
            if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                continue;
            }

            let absolute = entry.path();
            let relative = absolute.strip_prefix(&root).unwrap_or(absolute);
            let rel_str = relative.to_string_lossy();
            let abs_str = absolute.to_string_lossy();

            if compiled
                .iter()
                .any(|pat| pat.matches(rel_str.as_ref()) || pat.matches(abs_str.as_ref()))
            {
                paths.push(absolute.to_path_buf());
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

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn entry(&self) -> Option<&str> {
        self.entry.as_deref()
    }

    pub fn language_field(&self) -> Option<&str> {
        self.language.as_deref()
    }

    pub fn stages(&self) -> Option<&[String]> {
        self.stages.as_deref()
    }

    pub fn additional_dependencies(&self) -> Option<&[String]> {
        self.additional_dependencies.as_deref()
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

    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
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
        let version = self.version.as_deref().unwrap_or("latest");
        format!(
            "lang={} target={} entry={} version={}",
            self.language.as_str(),
            target,
            entry,
            version
        )
    }
}

impl InstallLanguage {
    fn as_str(self) -> &'static str {
        match self {
            InstallLanguage::Rust => "rust",
            InstallLanguage::Python => "python",
            InstallLanguage::Node => "node",
            InstallLanguage::Go => "go",
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
    let hooks = cfg.local_hooks();
    if hooks.is_empty() {
        return Err(anyhow!("No local hooks configured"));
    }

    for (_, h) in hooks {
        let enabled = h.enabled.unwrap_or(true);
        if !enabled {
            continue;
        }

        // Build list of matching files
        let paths = collect_files(h.files.as_ref())?;

        if paths.is_empty() {
            if ctx.debug {
                eprintln!("Skipping hook {}: no matching files", h.id());
            }
            continue;
        }

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

pub fn ensure_installed(ctx: &RunContext, hook: &HookConfig) -> Result<PathBuf> {
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
        InstallLanguage::Go => install_go(ctx, hook, install, &root)?,
    };

    if !path.exists() {
        anyhow::bail!(
            "Expected executable for hook '{}' at {} but it does not exist",
            hook.id,
            path.display()
        );
    }

    let language = install.language().as_str();
    let source_string = if let Some(pkg) = install.package() {
        if let Some(ver) = install.version() {
            Some(format!("package:{pkg}@{ver}"))
        } else {
            Some(format!("package:{pkg}"))
        }
    } else if let Some(repo) = install.repo() {
        if let Some(ver) = install.version() {
            Some(format!("repo:{repo}@{ver}"))
        } else {
            Some(format!("repo:{repo}"))
        }
    } else {
        None
    };
    lock::record_hook(
        hook.id(),
        language,
        source_string.as_deref(),
        Some(install.entry(hook.id())),
        &path,
    )?;

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

    if let Some(ver) = install.version() {
        if install.package().is_some() && install.repo().is_none() {
            cmd.arg("--version").arg(ver);
        }
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
        .map(|s| {
            if let Some(ver) = install.version() {
                if s.contains("==") {
                    s.to_string()
                } else {
                    format!("{s}=={ver}")
                }
            } else {
                s.to_string()
            }
        })
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

    if let Some(pkg) = install.package() {
        let target_spec = if let Some(ver) = install.version() {
            format!("{}@{}", pkg, ver)
        } else {
            pkg.to_string()
        };
        cmd.arg(target_spec);
    } else {
        cmd.arg(target);
    }

    run_and_check(cmd, ctx, "npm install")?;

    Ok(bin_path)
}

fn install_go(
    ctx: &RunContext,
    hook: &HookConfig,
    install: &InstallConfig,
    root: &Path,
) -> Result<PathBuf> {
    let entry = install.entry(hook.id());
    let bin_path = root.join("bin").join(entry);
    if bin_path.exists() {
        return Ok(bin_path);
    }

    fs::create_dir_all(root.join("bin"))?;

    let package = install.package().ok_or_else(|| {
        anyhow!(
            "Install for hook '{}' requires 'package' (module path)",
            hook.id
        )
    })?;

    if ctx.debug {
        eprintln!(
            "Installing go hook '{}' from {} into {}",
            hook.id,
            package,
            root.display()
        );
    }

    let mut cmd = Command::new("go");
    cmd.env("GOBIN", root.join("bin")).arg("install");

    if let Some(args) = install.install_args() {
        cmd.args(args);
    }

    let package_spec = if let Some(ver) = install.version() {
        format!("{}@{}", package, ver)
    } else if package.contains('@') {
        package.to_string()
    } else {
        anyhow::bail!(
            "Install for hook '{}' requires a version for Go modules; set install.version or include '@<version>' in package",
            hook.id
        );
    };

    cmd.arg(&package_spec);

    run_and_check(cmd, ctx, "go install")?;

    if !bin_path.exists() {
        anyhow::bail!(
            "After go install, expected binary {} but it was not created",
            bin_path.display()
        );
    }

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
        "# Ensure `uv`, `npm`, `cargo`, and `go` are available on PATH before running the respective external hooks.",
        "repos:",
        "  - repo: local",
        "    hooks:",
        "      - id: trailing-whitespace",
        "        name: trailing-whitespace",
        "        entry: trailing-whitespace",
        "        language: system",
        "        pass_filenames: true",
        "        files: '**/*.{rs,py,js,ts,txt,md}'",
        "      - id: end-of-file-fixer",
        "        name: end-of-file-fixer",
        "        entry: end-of-file-fixer",
        "        language: system",
        "        files: '**/*.{rs,py,txt,md}'",
        "      - id: check-yaml",
        "        name: check-yaml",
        "        entry: check-yaml",
        "        language: system",
        "        files: '**/*.{yml,yaml}'",
        "      - id: pretty-format-json",
        "        name: pretty-format-json",
        "        entry: pretty-format-json",
        "        language: system",
        "        files: '**/*.{json,jsonc}'",
        "      - id: check-added-large-files",
        "        name: check-added-large-files",
        "        entry: check-added-large-files",
        "        language: system",
        "        args: ['500000']  # optional max size in bytes",
        "",
        "      # Example hooks (uncomment to enable):",
        "      # - id: ruff-check",
        "      #   name: ruff-check",
        "      #   entry: ruff",
        "      #   language: system",
        "      #   command: \"{install}\"",
        "      #   files: '**/*.py'",
        "      #   install:",
        "      #     language: python",
        "      #     package: ruff",
        "      #     entry: ruff",
        "      #   args: ['check', '--fix']",
        "",
        "      # - id: prettier",
        "      #   name: prettier",
        "      #   entry: prettier",
        "      #   language: system",
        "      #   command: \"{install}\"",
        "      #   files: '**/*.{js,ts,jsx,tsx,json,css,md}'",
        "      #   install:",
        "      #     language: node",
        "      #     package: prettier",
        "      #     entry: prettier",
        "      #   args: ['--write']",
        "",
        "      # - id: cargo-deny",
        "      #   name: cargo-deny",
        "      #   entry: cargo-deny",
        "      #   language: system",
        "      #   command: \"{install}\"",
        "      #   files: '**/Cargo.lock'",
        "      #   install:",
        "      #     language: rust",
        "      #     package: cargo-deny",
        "      #     binary: cargo-deny",
        "      #   args: ['check']",
        "",
        "      # - id: golangci-lint",
        "      #   name: golangci-lint",
        "      #   entry: golangci-lint",
        "      #   language: system",
        "      #   command: \"{install}\"",
        "      #   files: '**/*.go'",
        "      #   install:",
        "      #     language: go",
        "      #     package: github.com/golangci/golangci-lint/cmd/golangci-lint",
        "      #     version: v1.61.0",
        "      #     entry: golangci-lint",
        "      #   args: ['run', '--fix']",
    ];
    let mut sample = lines.join("\n");
    sample.push('\n');
    std::fs::write(path, sample)?;
    Ok(())
}
