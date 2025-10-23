use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct LockFile {
    pub version: u32,
    pub generated_at: String,
    pub hooks: Vec<LockEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LockEntry {
    pub id: String,
    pub binary: String,
    pub sha256: String,
    pub language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
}

impl Default for LockFile {
    fn default() -> Self {
        LockFile {
            version: 1,
            generated_at: DateTime::<Utc>::from(Utc::now())
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            hooks: Vec::new(),
        }
    }
}

fn load_lock(path: &Path) -> Result<LockFile> {
    if path.exists() {
        let data = fs::read(path)?;
        let mut lock: LockFile = serde_yaml::from_slice(&data)?;
        lock.generated_at =
            DateTime::<Utc>::from(Utc::now()).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        Ok(lock)
    } else {
        Ok(LockFile::default())
    }
}

fn save_lock(path: &Path, lock: &LockFile) -> Result<()> {
    let yaml = serde_yaml::to_string(lock)?;
    fs::write(path, yaml)?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    io::copy(&mut reader, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Record information about an installed hook binary in `.precommit-lock.yaml`.
pub fn record_hook(
    id: &str,
    language: &str,
    source: Option<&str>,
    entry: Option<&str>,
    binary_path: &Path,
) -> Result<()> {
    let root = std::env::current_dir()?;
    let lock_path = root.join(".precommit-lock.yaml");

    let mut lock = load_lock(&lock_path)?;

    let binary_rel = binary_path
        .strip_prefix(&root)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| binary_path.to_string_lossy().to_string());

    let sha256 = sha256_file(binary_path)?;

    lock.hooks.retain(|entry| entry.id != id);
    lock.hooks.push(LockEntry {
        id: id.to_string(),
        binary: binary_rel,
        sha256,
        language: language.to_string(),
        source: source.map(|s| s.to_string()),
        entry: entry.map(|s| s.to_string()),
    });
    lock.hooks.sort_by(|a, b| a.id.cmp(&b.id));

    save_lock(&lock_path, &lock)?;
    Ok(())
}
