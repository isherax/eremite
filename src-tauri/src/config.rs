use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelRef {
    pub repo_id: String,
    pub filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub last_used_model: Option<ModelRef>,
}

impl AppConfig {
    pub fn empty() -> Self {
        Self {
            last_used_model: None,
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::empty());
        }
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;
        let config: AppConfig = serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse config at {}", path.display()))?;
        Ok(config)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(path, contents)
            .with_context(|| format!("failed to write config to {}", path.display()))?;
        Ok(())
    }

    pub fn default_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("could not determine home directory")?;
        Ok(home.join(".eremite").join("config.json"))
    }

    pub fn load_default() -> Self {
        Self::default_path()
            .and_then(|p| Self::load(&p))
            .unwrap_or_else(|_| Self::empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn empty_config_round_trip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.json");

        let config = AppConfig::empty();
        config.save(&path).unwrap();

        let loaded = AppConfig::load(&path).unwrap();
        assert!(loaded.last_used_model.is_none());
    }

    #[test]
    fn config_with_model_round_trip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.json");

        let config = AppConfig {
            last_used_model: Some(ModelRef {
                repo_id: "bartowski/Llama-3.2-1B-Instruct-GGUF".to_string(),
                filename: "Llama-3.2-1B-Instruct-Q4_K_M.gguf".to_string(),
            }),
        };
        config.save(&path).unwrap();

        let loaded = AppConfig::load(&path).unwrap();
        assert_eq!(loaded.last_used_model, config.last_used_model);
    }

    #[test]
    fn load_nonexistent_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("does_not_exist.json");

        let config = AppConfig::load(&path).unwrap();
        assert!(config.last_used_model.is_none());
    }

    #[test]
    fn future_fields_ignored() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.json");

        std::fs::write(&path, r#"{"last_used_model": null, "unknown_field": 42}"#).unwrap();

        let config = AppConfig::load(&path).unwrap();
        assert!(config.last_used_model.is_none());
    }
}
