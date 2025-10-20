# precommit-rs

This repository is a small Rust translation of a subset of hooks from https://github.com/pre-commit/pre-commit-hooks.

Included hooks (partial):

- trailing-whitespace: remove trailing spaces and tabs
- end-of-file-fixer: ensure files end with a single newline
- check-added-large-files: warn/exit if files exceed a size limit
- check-yaml: validate YAML files
- pretty-format-json: pretty-print JSON files

Usage

Build and run the CLI (examples):

```bash
cargo build --release
# fix trailing whitespace:
./target/release/precommit-rs trailing-whitespace path/to/files
# ensure EOF newline:
./target/release/precommit-rs end-of-file-fixer path/to/files
# check yaml:
./target/release/precommit-rs check-yaml path/to/files
# format json:
./target/release/precommit-rs pretty-format-json path/to/files
```

Notes

- Hooks that modify files exit with status code 1 when they make changes (so orchestration tools like pre-commit can re-stage files).
- This is an initial translation and does not implement every hook or every option from the original project.

Next steps

- Add more hooks from the upstream project
- Add integration tests and better CLI options (e.g., pattern matching, recursive flags)
- Consider producing separate binaries for each hook to match pre-commit's expectations

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
