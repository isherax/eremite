pub mod download;
pub mod manifest;
pub mod search;

pub use search::{
    default_hub_origin, popular_gguf_models, search_gguf_models, GgufFileInfo, SearchResult,
};

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use chrono::Utc;

use crate::download::{build_download_url, default_base_url, download_file};
use crate::manifest::{Manifest, ModelEntry};

pub struct ModelManager {
    base_path: PathBuf,
    manifest: Manifest,
}

impl ModelManager {
    /// Create a `ModelManager` rooted at the given directory.
    /// Loads the manifest from `<base_path>/models/manifest.json` if it exists.
    pub fn new(base_path: impl Into<PathBuf>) -> Result<Self> {
        let base_path = base_path.into();
        let manifest = Manifest::load(&Self::manifest_path_for(&base_path))?;
        Ok(Self {
            base_path,
            manifest,
        })
    }

    /// Create a `ModelManager` using the default path (`~/.eremite/`).
    pub fn default_path() -> Result<Self> {
        let home = dirs::home_dir().context("could not determine home directory")?;
        Self::new(home.join(".eremite"))
    }

    /// Download a GGUF model file from Hugging Face Hub (or a custom base URL).
    pub async fn download(
        &mut self,
        repo_id: &str,
        filename: &str,
        base_url: Option<&str>,
    ) -> Result<ModelEntry> {
        let base_url = base_url.unwrap_or_else(|| default_base_url());
        let url = build_download_url(base_url, repo_id, filename);
        let dest = self.model_path(repo_id, filename);

        let result = download_file(&url, &dest, |_, _| {}).await?;

        let entry = ModelEntry {
            repo_id: repo_id.to_string(),
            filename: filename.to_string(),
            size_bytes: result.size_bytes,
            sha256: result.sha256,
            downloaded_at: Utc::now(),
        };

        self.manifest.add(entry.clone());
        self.manifest.save(&self.manifest_path())?;

        Ok(entry)
    }

    /// Download with a progress callback.
    /// `on_progress` receives (bytes_downloaded, total_bytes_option) after each chunk.
    pub async fn download_with_progress(
        &mut self,
        repo_id: &str,
        filename: &str,
        base_url: Option<&str>,
        on_progress: impl Fn(u64, Option<u64>),
    ) -> Result<ModelEntry> {
        let base_url = base_url.unwrap_or_else(|| default_base_url());
        let url = build_download_url(base_url, repo_id, filename);
        let dest = self.model_path(repo_id, filename);

        let result = download_file(&url, &dest, on_progress).await?;

        let entry = ModelEntry {
            repo_id: repo_id.to_string(),
            filename: filename.to_string(),
            size_bytes: result.size_bytes,
            sha256: result.sha256,
            downloaded_at: Utc::now(),
        };

        self.manifest.add(entry.clone());
        self.manifest.save(&self.manifest_path())?;

        Ok(entry)
    }

    /// List all downloaded models.
    pub fn list(&self) -> &[ModelEntry] {
        &self.manifest.models
    }

    /// Look up a single model by repo_id and filename.
    pub fn get(&self, repo_id: &str, filename: &str) -> Option<&ModelEntry> {
        self.manifest.find(repo_id, filename)
    }

    /// Remove a downloaded model. Deletes the file and removes the manifest entry.
    pub fn remove(&mut self, repo_id: &str, filename: &str) -> Result<()> {
        let path = self.model_path(repo_id, filename);
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to delete {}", path.display()))?;
        }

        if !self.manifest.remove_entry(repo_id, filename) {
            bail!("model {}/{} not found in manifest", repo_id, filename);
        }

        self.manifest.save(&self.manifest_path())?;
        Ok(())
    }

    /// Return the on-disk path where a model file lives (or would live).
    pub fn model_path(&self, repo_id: &str, filename: &str) -> PathBuf {
        self.models_dir().join(repo_id).join(filename)
    }

    fn models_dir(&self) -> &Path {
        // models/ is directly under base_path
        // We return base_path/models but as a PathBuf since we join onto it
        &self.base_path
    }

    fn manifest_path(&self) -> PathBuf {
        Self::manifest_path_for(&self.base_path)
    }

    fn manifest_path_for(base_path: &Path) -> PathBuf {
        base_path.join("models").join("manifest.json")
    }
}
