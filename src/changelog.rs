use anyhow::{Context, Result};
use chrono::Local;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Default, Debug)]
pub struct ChangelogEntry {
    pub hook_id: String,
    pub changes: Vec<String>,
    pub files_checked: Vec<PathBuf>,
    pub files_modified: Vec<PathBuf>,
}

#[derive(Debug)]
pub struct Changelog {
    entries: HashMap<String, ChangelogEntry>,
    has_changes: bool,
}

impl Changelog {
    pub fn new() -> Self {
        Changelog {
            entries: HashMap::new(),
            has_changes: false,
        }
    }

    pub fn add_entry(&mut self, hook_id: &str) -> &mut ChangelogEntry {
        self.entries
            .entry(hook_id.to_string())
            .or_insert_with(|| ChangelogEntry {
                hook_id: hook_id.to_string(),
                ..Default::default()
            })
    }

    pub fn record_change(&mut self, hook_id: &str, message: &str) {
        let entry = self.add_entry(hook_id);
        entry.changes.push(message.to_string());
        self.has_changes = true;
    }

    pub fn record_file_checked(&mut self, hook_id: &str, path: &Path) {
        let entry = self.add_entry(hook_id);
        entry.files_checked.push(path.to_path_buf());
    }

    pub fn record_file_modified(&mut self, hook_id: &str, path: &Path) {
        let entry = self.add_entry(hook_id);
        entry.files_modified.push(path.to_path_buf());
        self.has_changes = true;
    }

    pub fn has_changes(&self) -> bool {
        self.has_changes
    }

    pub fn write_if_changed(&self) -> Result<()> {
        eprintln!("Checking for changes to write to changelog");
        if !self.has_changes {
            eprintln!("No changes to write to changelog");
            return Ok(());
        }
        eprintln!("Writing changes to changelog...");

        let now = Local::now();
        let date_str = now.format("%Y-%m-%d %H:%M:%S");

        let mut content = format!("# Pre-commit Changes {}\n\n", date_str);

        for entry in self.entries.values() {
            if entry.changes.is_empty() && entry.files_modified.is_empty() {
                continue;
            }

            content.push_str(&format!("## Hook: {}\n\n", entry.hook_id));

            if !entry.changes.is_empty() {
                content.push_str("### Changes:\n");
                for change in &entry.changes {
                    content.push_str(&format!("- {}\n", change));
                }
                content.push('\n');
            }

            if !entry.files_modified.is_empty() {
                content.push_str("### Modified Files:\n");
                for file in &entry.files_modified {
                    content.push_str(&format!("- `{}`\n", file.display()));
                }
                content.push('\n');
            }

            let unmodified: Vec<_> = entry
                .files_checked
                .iter()
                .filter(|f| !entry.files_modified.contains(f))
                .collect();

            if !unmodified.is_empty() {
                content.push_str("### Checked Files (no changes):\n");
                for file in unmodified {
                    content.push_str(&format!("- `{}`\n", file.display()));
                }
                content.push('\n');
            }
        }

        // Read existing changelog if it exists
        let changelog_path = Path::new("PRECOMMIT_CHANGELOG.md");
        let existing = if changelog_path.exists() {
            fs::read_to_string(changelog_path)?
        } else {
            String::new()
        };

        // Prepend new changes to existing content
        let full_content = if existing.is_empty() {
            content
        } else {
            format!("{}\n---\n\n{}", content, existing)
        };

        fs::write(changelog_path, full_content).context("Failed to write changelog")?;

        Ok(())
    }
}
