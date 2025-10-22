# precommit-rs

Deterministic, cross-platform binaries make it easy to use the hooks directly, wrap them with `pre-commit`, or package them for distribution.

## Platforms & Installation

GitHub Actions builds release artifacts for Linux, macOS, and Windows. Download the ZIP/DMG/AppImage/pkg appropriate for your platform from the latest release.

### macOS

1. Download `precommit-rs-macos.zip` (contains a notarization-ready `.app`) or the `.dmg` installer.
2. Unzip/mount and drag `precommit-rs.app` into `/Applications` (or invoke the CLI within the bundle: `precommit-rs.app/Contents/MacOS/precommit-rs`).
3. Optional: add the CLI to your PATH for scripts:

   ```bash
   sudo ln -sf "/Applications/precommit-rs.app/Contents/MacOS/precommit-rs" /usr/local/bin/precommit-rs
   ```

### Linux

Two formats ship for Linux:

- `precommit-rs-x86_64.AppImage` — Run `chmod +x precommit-rs-*.AppImage && ./precommit-rs-*.AppImage`. You can copy the mounted binary to your PATH or integrate the AppImage directly.
- `precommit-rs_*.deb` and `precommit-rs-*.rpm` — Install with your package manager:

  ```bash
  sudo dpkg -i precommit-rs_*.deb       # Debian/Ubuntu
  sudo rpm -i precommit-rs-*.rpm         # Fedora/openSUSE
  ```

The standalone musl build (`precommit-rs` inside the tarball) runs on most glibc-compatible distributions.

### Windows

You’ll find multiple options:

- `precommit-rs-installer.exe` — NSIS installer that adds the binary to your PATH.
- Portable `.zip` — unzip and use `precommit-rs.exe` directly.

All hook-specific executables also ship for convenience (e.g., `trailing-whitespace.exe`).

### Build From Source

```bash
git clone https://github.com/<org>/precommit-rs.git
cd precommit-rs
cargo build --release --bins
# binaries land in target/release/
```

## CLI Overview

```
precommit-rs: precommit hook framework and small collection of pre-commit hooks in Rust

Usage:  precommit-rs [OPTIONS] <COMMAND>

Commands:
  trailing-whitespace      Fix trailing whitespace in files
  end-of-file-fixer        Ensure file ends with a single newline
  check-added-large-files  Fail if added files exceed a size limit (in bytes)
  check-yaml               Validate YAML files
  pretty-format-json       Pretty-format JSON files (in-place)
  completions              Generate shell completion scripts
  list-hooks               List hooks from configuration
  run-config               Read a pre-commit YAML config file and run the enabled hooks
  init                     Create a default .pre-commit.yaml in the current directory (or specified path)
  install                  Install a git pre-commit hook in the repository that runs precommit-rs
  create-hook              Create a new custom pre-commit hook from a template
  validate-hook            Validate that a hook implementation meets the required contract
  help                     Print this message or the help of the given subcommand(s)

Options:
      --dry-run  Do not write changes, only report what would be changed
      --debug    Enable debug output
  -h, --help     Print help
  -V, --version  Print version
```

### Hook Binaries

Each built-in hook is also compiled as a dedicated binary (e.g., `trailing-whitespace`) for users who prefer one-tool-per-hook.

## Quick Start

### Run Built-in Hooks Directly

```bash
precommit-rs trailing-whitespace src/
precommit-rs end-of-file-fixer README.md
precommit-rs check-yaml config/*.yml
precommit-rs pretty-format-json data/**/*.json
precommit-rs check-added-large-files --max-bytes 1048576 .
```

Hooks exit with code `1` if they modify files or encounter errors—mirroring standard pre-commit semantics.

### Generate Completions

```bash
precommit-rs completions --shell bash --out precommit-rs.bash
source precommit-rs.bash
```

### Bootstrap Configuration

```bash
# Create a starter .pre-commit.yaml with examples
precommit-rs init

# List hooks enabled in the config
precommit-rs list-hooks

# Install a git pre-commit hook that runs the config
precommit-rs install

# Execute all configured hooks manually
precommit-rs run-config
```

External hooks defined in `.pre-commit.yaml` with `command: "{install}"` are installed automatically on first use. Python hooks use `uv venv` to provision a dedicated environment; Node hooks rely on `npm install`, and Rust hooks use `cargo install`.

## Included Hooks (Summary)

- `trailing-whitespace` — remove trailing spaces and tabs.
- `end-of-file-fixer` — ensure text files end with a single newline.
- `check-added-large-files` — warn/fail if files exceed a size limit.
- `check-yaml` — parse/validate YAML documents.
- `pretty-format-json` — format JSON files deterministically.

New hooks live under `src/hooks/`, with runnable wrappers in `crates/<hook>/`.

macOS signing for CI

If you want the GitHub Actions workflow to codesign macOS bundles, add two repository secrets:

- `MAC_SIGNING_P12` — a base64-encoded `.p12` certificate (single-line, no newlines). Create it locally with:

```bash
base64 /path/to/your/certificate.p12 | tr -d '\n'
```

- `MAC_SIGNING_PASSWORD` — the password for the `.p12` file.

Behavior in CI:

- The workflow will only attempt to sign on macOS runners. The signing step is conditional and will be skipped (successfully) if either secret is missing. This means builds will still produce unsigned artifacts when you haven't configured signing.
- When both secrets are present, the workflow decodes the P12, imports it into a temporary keychain, attempts to codesign `.app` bundles under `target/release/bundle/macos/`, verifies them, and cleans up the temporary keychain.

Notes:

- Notarization is not performed by default. If you need notarization, you'll need to provide Apple notarization credentials and add corresponding steps to the workflow.
- Keep your P12 and password secure — add them only to GitHub repository (or organization) secrets and do not commit them to source control.
