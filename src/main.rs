use anyhow::anyhow;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use std::{fs::File, io, path::PathBuf};

use precommit_rs::{cli, config, hooks, RunContext};

const COLOR_RESET: &str = "\x1b[0m";
const COLOR_REPO: &str = "\x1b[1;34m";
const COLOR_HOOK_ENABLED: &str = "\x1b[1;36m";
const COLOR_HOOK_DISABLED: &str = "\x1b[2;36m";
const COLOR_STATUS_ENABLED: &str = "\x1b[32m";
const COLOR_STATUS_DISABLED: &str = "\x1b[2;31m";
const COLOR_KIND_BUILTIN: &str = "\x1b[34m";
const COLOR_KIND_EXTERNAL: &str = "\x1b[35m";
const COLOR_COMMAND: &str = "\x1b[32m";
const COLOR_FILES: &str = "\x1b[33m";
const COLOR_NOTE: &str = "\x1b[2;37m";
const COLOR_INSTALL: &str = "\x1b[35m";

#[derive(Clone, ValueEnum, Debug)]
enum HookLanguage {
    Rust,
    Python,
    Shell,
}

#[derive(Parser)]
#[command(
    author,
    version,
    about = "precommit-rs: precommit hook framework and small collection of pre-commit hooks in Rust",
    color = clap::ColorChoice::Always,
    styles = cli::styles()
)]
struct Cli {
    /// Do not write changes, only report what would be changed
    #[arg(long, global = true)]
    dry_run: bool,

    /// Enable debug output
    #[arg(long, global = true)]
    debug: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Fix trailing whitespace in files
    TrailingWhitespace { paths: Vec<PathBuf> },
    /// Ensure file ends with a single newline
    EndOfFileFixer { paths: Vec<PathBuf> },
    /// Fail if added files exceed a size limit (in bytes)
    CheckAddedLargeFiles {
        max_bytes: Option<u64>,
        paths: Vec<PathBuf>,
    },
    /// Validate YAML files
    CheckYaml { paths: Vec<PathBuf> },
    /// Pretty-format JSON files (in-place)
    PrettyFormatJson { paths: Vec<PathBuf> },
    /// Generate shell completion scripts
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, powershell, elvish)
        #[arg(value_enum)]
        shell: Shell,
        /// Output path for the completion script (defaults to stdout)
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// List hooks from configuration
    ListHooks {
        /// Path to configuration file (default: .pre-commit.yaml)
        #[arg(long)]
        config: Option<PathBuf>,
        /// Include disabled hooks in the output
        #[arg(long)]
        all: bool,
    },
    /// Read a pre-commit YAML config file and run the enabled hooks
    RunConfig { config: Option<PathBuf> },
    /// Create a default .pre-commit.yaml in the current directory (or specified path)
    Init { path: Option<PathBuf> },
    /// Install a git pre-commit hook in the repository that runs precommit-rs
    Install {
        /// Path to the precommit-rs binary to use (optional)
        #[arg(long)]
        path: Option<String>,
    },
    /// Create a new custom pre-commit hook from a template
    CreateHook {
        /// The name of your hook (e.g. "check-todo")
        name: String,
        /// Programming language to use (rust, python, or shell)
        #[arg(value_enum)]
        language: HookLanguage,
        /// Short description of what the hook does
        description: String,
        /// Directory to create the hook in (default: current directory)
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },
    /// Validate that a hook implementation meets the required contract
    ValidateHook {
        /// The name of the hook to validate (e.g. "end-of-file-fixer")
        hook_name: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut ctx = RunContext::default();
    ctx.dry_run = cli.dry_run;
    ctx.debug = cli.debug;

    match cli.command {
        Commands::TrailingWhitespace { paths } => hooks::trailing_whitespace::run_with_ctx(&ctx, paths),
        Commands::EndOfFileFixer { paths } => hooks::end_of_file::run_with_ctx(&ctx, paths),
        Commands::CheckAddedLargeFiles { max_bytes, paths } => hooks::check_added_large_files::run_with_ctx(&ctx, max_bytes, paths),
        Commands::CheckYaml { paths } => hooks::check_yaml::run_with_ctx(&ctx, paths),
        Commands::PrettyFormatJson { paths } => hooks::pretty_format_json::run_with_ctx(&ctx, paths),
        Commands::Completions { shell, out } => {
            let mut cmd = Cli::command();
            let bin_name = cmd.get_name().to_string();

            if let Some(path) = out {
                let mut file = File::create(&path)?;
                clap_complete::generate(shell, &mut cmd, bin_name, &mut file);
                println!(
                    "Wrote {} completions to {}",
                    shell.to_string(),
                    path.display()
                );
            } else {
                let mut stdout = io::stdout();
                clap_complete::generate(shell, &mut cmd, bin_name, &mut stdout);
            }
            Ok(())
        }
        Commands::RunConfig { config } => {
            let cfg_path = config.unwrap_or_else(|| PathBuf::from(".pre-commit.yaml"));
            let conf = config::PreCommitConfig::from_file(&cfg_path)?;
            if ctx.debug {
                eprintln!("Loaded config from {}: {:#?}", cfg_path.display(), conf);
            }
            config::run_config(&ctx, &conf)?;
            Ok(())
        }
        Commands::ListHooks { config, all } => {
            let cfg_path = config.unwrap_or_else(|| PathBuf::from(".pre-commit.yaml"));
            let conf = config::PreCommitConfig::from_file(&cfg_path)?;
            let repos = conf.repos();

            if repos.is_empty() {
                println!("No repos configured in {}", cfg_path.display());
                return Ok(());
            }

            let scope_label = if all { "including disabled" } else { "enabled only" };
            println!(
                "{}Hooks in{} {} ({})",
                COLOR_REPO,
                COLOR_RESET,
                cfg_path.display(),
                scope_label
            );

            for repo in repos {
                if repo.hooks().is_empty() {
                    continue;
                }
                let rev_suffix = repo
                    .rev()
                    .map(|rev| format!(" @{}{}{}", COLOR_NOTE, rev, COLOR_RESET))
                    .unwrap_or_default();
                println!(
                    "{}repo:{} {}{}",
                    COLOR_REPO,
                    COLOR_RESET,
                    repo.repo(),
                    rev_suffix
                );
                for hook in repo.hooks() {
                    if !all && !hook.is_enabled() {
                        continue;
                    }

                    let (id_color, status_label, kind_label) = if hook.is_enabled() {
                        (
                            COLOR_HOOK_ENABLED,
                            format!("{}{status}{}", COLOR_STATUS_ENABLED, COLOR_RESET, status = "enabled"),
                            if hook.is_builtin() {
                                format!("{}{kind}{}", COLOR_KIND_BUILTIN, COLOR_RESET, kind = "builtin")
                            } else {
                                format!("{}{kind}{}", COLOR_KIND_EXTERNAL, COLOR_RESET, kind = "external")
                            },
                        )
                    } else {
                        (
                            COLOR_HOOK_DISABLED,
                            format!("{}{status}{}", COLOR_STATUS_DISABLED, COLOR_RESET, status = "disabled"),
                            if hook.is_builtin() {
                                format!("{}{kind}{}", COLOR_KIND_BUILTIN, COLOR_RESET, kind = "builtin")
                            } else {
                                format!("{}{kind}{}", COLOR_KIND_EXTERNAL, COLOR_RESET, kind = "external")
                            },
                        )
                    };
                    let install_note = if hook.command_is_install() {
                        hook.install()
                            .map(|inst| {
                                format!(
                                    " {}[install: {}]{}",
                                    COLOR_INSTALL,
                                    inst.summary(),
                                    COLOR_RESET
                                )
                            })
                            .unwrap_or_else(|| {
                                format!(
                                    " {}[install: missing config]{}",
                                    COLOR_INSTALL,
                                    COLOR_RESET
                                )
                            })
                    } else {
                        String::new()
                    };
                    let entry_note = hook
                        .entry()
                        .map(|e| format!(" {}[entry: {}]{}", COLOR_NOTE, e, COLOR_RESET))
                        .unwrap_or_default();
                    let language_note = hook
                        .language_field()
                        .map(|l| format!(" {}[language: {}]{}", COLOR_NOTE, l, COLOR_RESET))
                        .unwrap_or_default();
                    let stages_note = hook
                        .stages()
                        .filter(|s| !s.is_empty())
                        .map(|s| format!(" {}[stages: {}]{}", COLOR_NOTE, s.join(","), COLOR_RESET))
                        .unwrap_or_default();
                    let deps_note = hook
                        .additional_dependencies()
                        .filter(|d| !d.is_empty())
                        .map(|d| format!(" {}[deps: {}]{}", COLOR_NOTE, d.join(","), COLOR_RESET))
                        .unwrap_or_default();

                    if let Some(cmd) = hook.command() {
                        println!(
                            "  - {}{}{} ({}, {}) -> {}{}{}{}{}{}{}",
                            id_color,
                            hook.id(),
                            COLOR_RESET,
                            status_label,
                            kind_label,
                            format!("{}{cmd}{}", COLOR_COMMAND, COLOR_RESET, cmd = cmd),
                            hook
                                .args()
                                .map(|args| format!(" {}{}{}", COLOR_COMMAND, args.join(" "), COLOR_RESET))
                                .unwrap_or_default(),
                            install_note,
                            entry_note,
                            language_note,
                            stages_note,
                            deps_note
                        );
                    } else {
                        println!(
                            "  - {}{}{} ({}, {}){}{}{}{}{}{}",
                            id_color,
                            hook.id(),
                            COLOR_RESET,
                            status_label,
                            kind_label,
                            hook
                                .files()
                                .map(|f| format!(" {}[files: {}]{}", COLOR_FILES, f, COLOR_RESET))
                                .unwrap_or_default(),
                            install_note,
                            entry_note,
                            language_note,
                            stages_note,
                            deps_note
                        );
                    }
                }
            }

            Ok(())
        }
        Commands::Init { path } => {
            let p = path.unwrap_or_else(|| PathBuf::from(".pre-commit.yaml"));
            config::write_default_config(&p)?;
            println!("Wrote default config to {}", p.display());
            Ok(())
        }
        Commands::ValidateHook { hook_name } => {
            match hook_name.as_str() {
                "end-of-file-fixer" => precommit_rs::validate::validate_hook("end-of-file-fixer", hooks::end_of_file::run_with_ctx),
                "trailing-whitespace" => precommit_rs::validate::validate_hook("trailing-whitespace", hooks::trailing_whitespace::run_with_ctx),
                "check-yaml" => precommit_rs::validate::validate_hook("check-yaml", hooks::check_yaml::run_with_ctx),
                "pretty-format-json" => precommit_rs::validate::validate_hook("pretty-format-json", hooks::pretty_format_json::run_with_ctx),
                _ => Err(anyhow!("Unknown hook: {}. Available hooks: end-of-file-fixer, trailing-whitespace, check-yaml, pretty-format-json", hook_name)),
            }
        }
        Commands::CreateHook { name, language, description, output_dir } => {
            let output_dir = output_dir.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let hook_dir = output_dir.join(&name);

            if hook_dir.exists() {
                if !hook_dir.is_dir() {
                    return Err(anyhow!("{} exists but is not a directory", hook_dir.display()));
                }
                println!("Hook directory {} already exists, updating...", hook_dir.display());
            } else {
                std::fs::create_dir_all(&hook_dir)?;
            }

            // Read appropriate template
            let template_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("templates");

            match language {
                HookLanguage::Rust => {
                    // Create Rust project structure
                    std::fs::create_dir_all(hook_dir.join("src"))?;

                    // Read and process templates
                    let cargo_template = std::fs::read_to_string(template_dir.join("rust_cargo.template"))?
                        .replace("{{hook_name}}", &name);
                    let main_template = std::fs::read_to_string(template_dir.join("rust_hook.template"))?
                        .replace("{{hook_name}}", &name)
                        .replace("{{description}}", &description);

                    // Write files
                    std::fs::write(hook_dir.join("Cargo.toml"), cargo_template)?;
                    std::fs::write(hook_dir.join("src").join("main.rs"), main_template)?;
                }
                HookLanguage::Python => {
                    let template = std::fs::read_to_string(template_dir.join("python_hook.template"))?
                        .replace("{{hook_name}}", &name)
                        .replace("{{description}}", &description);

                    let script_path = hook_dir.join(format!("{}.py", name));
                    std::fs::write(&script_path, template)?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))?;
                    }
                }
                HookLanguage::Shell => {
                    let template = std::fs::read_to_string(template_dir.join("shell_hook.template"))?
                        .replace("{{hook_name}}", &name)
                        .replace("{{description}}", &description);

                    let script_path = hook_dir.join(&name);
                    std::fs::write(&script_path, template)?;
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))?;
                    }
                }
            }

            // Create a sample pre-commit config
            let config = format!("\
# Add this to your .pre-commit.yaml to use this hook:
  - id: {}
    files: '**/*'  # Adjust pattern to match files you want to check
    enabled: true
    command: {}",
                name,
                hook_dir.join(match language {
                    HookLanguage::Rust => "target/release/".to_string() + &name,
                    HookLanguage::Python => format!("{}.py", name),
                    HookLanguage::Shell => name.clone(),
                }).display());

            std::fs::write(hook_dir.join("pre-commit-config.yaml"), config)?;

            println!("Created new pre-commit hook in {}", hook_dir.display());
            println!("For Rust hooks, run 'cargo build --release' in the hook directory before using");
            Ok(())
        }
        Commands::Install { path } => {
            // Find repo root
            let root_out = std::process::Command::new("git").args(["rev-parse", "--show-toplevel"]).output()?;
            let repo_root = String::from_utf8_lossy(&root_out.stdout).trim().to_string();
            let hook_path = PathBuf::from(&repo_root).join(".git/hooks/pre-commit");

            // Determine binary path:
            // 1. Use --path if provided
            // 2. Try `which precommit-rs`
            // 3. Fall back to ./target/release/precommit-rs
            let binary_path = if let Some(p) = path {
                p
            } else {
                // Try to find installed binary with `which`
                let which_out = std::process::Command::new("which")
                    .arg("precommit-rs")
                    .output();

                match which_out {
                    Ok(out) if out.status.success() => {
                        String::from_utf8_lossy(&out.stdout).trim().to_string()
                    }
                    _ => {
                        // Fall back to local release binary
                        let local_bin = format!("{}/target/release/precommit-rs", repo_root);
                        if ctx.debug {
                            eprintln!("No installed binary found, using {}", local_bin);
                        }
                        local_bin
                    }
                }
            };

            let script = format!(
                "#!/usr/bin/env bash\n\
                set -e\n\
                \n\
                # Run pre-commit hooks using {}\n\
                exec \"{}\" run-config\n",
                binary_path, binary_path
            );

            if ctx.debug {
                eprintln!("Writing hook script to use binary: {}", binary_path);
            }
            std::fs::write(&hook_path, script)?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&hook_path)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&hook_path, perms)?;
            }

            println!("Installed git hook at {} using binary: {}", hook_path.display(), binary_path);

            let cfg_path = PathBuf::from(&repo_root).join(".pre-commit.yaml");
            if cfg_path.exists() {
                if ctx.debug {
                    eprintln!("Ensuring external hooks are installed per {}", cfg_path.display());
                }
                let conf = config::PreCommitConfig::from_file(&cfg_path)?;
                for (_, hook) in conf.local_hooks() {
                    if hook.command_is_install() {
                        if ctx.debug {
                            eprintln!("Installing hook {} for lockfile", hook.id());
                        }
                        config::ensure_installed(&ctx, hook)?;
                    }
                }
                println!("Updated .precommit-lock.yaml with installed hook hashes.");
            } else if ctx.debug {
                eprintln!("No .pre-commit.yaml found at {}", cfg_path.display());
            }
            Ok(())
        }
    }
}
