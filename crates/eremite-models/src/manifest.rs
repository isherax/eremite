use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub models: Vec<ModelEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelEntry {
    pub repo_id: String,
    pub filename: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub downloaded_at: DateTime<Utc>,
}

impl Manifest {
    pub fn empty() -> Self {
        Self {
            version: 1,
            models: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::empty());
        }
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read manifest at {}", path.display()))?;
        let manifest: Manifest = serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse manifest at {}", path.display()))?;
        Ok(manifest)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(path, contents)
            .with_context(|| format!("failed to write manifest to {}", path.display()))?;
        Ok(())
    }

    pub fn add(&mut self, entry: ModelEntry) {
        self.remove_entry(&entry.repo_id, &entry.filename);
        self.models.push(entry);
    }

    pub fn find(&self, repo_id: &str, filename: &str) -> Option<&ModelEntry> {
        self.models
            .iter()
            .find(|e| e.repo_id == repo_id && e.filename == filename)
    }

    pub fn remove_entry(&mut self, repo_id: &str, filename: &str) -> bool {
        let len_before = self.models.len();
        self.models
            .retain(|e| !(e.repo_id == repo_id && e.filename == filename));
        self.models.len() != len_before
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_entry() -> ModelEntry {
        ModelEntry {
            repo_id: "test-org/test-model-GGUF".to_string(),
            filename: "test-model-Q4_K_M.gguf".to_string(),
            size_bytes: 1024,
            sha256: "abc123".to_string(),
            downloaded_at: Utc::now(),
        }
    }

    #[test]
    fn round_trip_serialize_deserialize() {
        let mut manifest = Manifest::empty();
        manifest.add(sample_entry());

        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let deserialized: Manifest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.version, 1);
        assert_eq!(deserialized.models.len(), 1);
        assert_eq!(deserialized.models[0].repo_id, "test-org/test-model-GGUF");
    }

    #[test]
    fn add_find_remove() {
        let mut manifest = Manifest::empty();
        let entry = sample_entry();

        manifest.add(entry.clone());
        assert_eq!(manifest.models.len(), 1);

        let found = manifest.find("test-org/test-model-GGUF", "test-model-Q4_K_M.gguf");
        assert!(found.is_some());
        assert_eq!(found.unwrap(), &entry);

        let removed = manifest.remove_entry("test-org/test-model-GGUF", "test-model-Q4_K_M.gguf");
        assert!(removed);
        assert_eq!(manifest.models.len(), 0);

        let not_removed = manifest.remove_entry("nonexistent/repo", "no-file.gguf");
        assert!(!not_removed);
    }

    #[test]
    fn duplicate_key_overwrites() {
        let mut manifest = Manifest::empty();

        let entry1 = sample_entry();
        manifest.add(entry1);

        let entry2 = ModelEntry {
            repo_id: "test-org/test-model-GGUF".to_string(),
            filename: "test-model-Q4_K_M.gguf".to_string(),
            size_bytes: 2048,
            sha256: "def456".to_string(),
            downloaded_at: Utc::now(),
        };
        manifest.add(entry2.clone());

        assert_eq!(manifest.models.len(), 1);
        assert_eq!(manifest.models[0].size_bytes, 2048);
        assert_eq!(manifest.models[0].sha256, "def456");
    }

    #[test]
    fn load_nonexistent_creates_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("does_not_exist.json");

        let manifest = Manifest::load(&path).unwrap();
        assert_eq!(manifest.version, 1);
        assert!(manifest.models.is_empty());
    }

    #[test]
    fn save_and_load_round_trip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("models/manifest.json");

        let mut manifest = Manifest::empty();
        manifest.add(sample_entry());
        manifest.save(&path).unwrap();

        let loaded = Manifest::load(&path).unwrap();
        assert_eq!(loaded.models.len(), 1);
        assert_eq!(loaded.models[0], manifest.models[0]);
    }
}
