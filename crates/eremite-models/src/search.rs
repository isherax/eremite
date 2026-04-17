//! Hugging Face Hub model search (public `/api/models` endpoint).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const DEFAULT_HUB_ORIGIN: &str = "https://huggingface.co";

/// Default origin for Hugging Face Hub (scheme + host, no trailing slash).
pub fn default_hub_origin() -> &'static str {
    DEFAULT_HUB_ORIGIN
}

/// One GGUF file listed under a model repo (from Hub `siblings`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GgufFileInfo {
    pub filename: String,
    pub size_bytes: Option<u64>,
    /// Parsed from the filename when possible (e.g. `Q4_K_M`), not hardware estimates.
    pub quantization_label: Option<String>,
}

/// A Hub model repo row suitable for discovery UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResult {
    pub repo_id: String,
    pub author: Option<String>,
    pub downloads: u64,
    pub likes: u64,
    pub tags: Vec<String>,
    pub gguf_files: Vec<GgufFileInfo>,
}

#[derive(Debug, Deserialize)]
struct HfSibling {
    rfilename: String,
    #[serde(default)]
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct HfModelItem {
    id: String,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    downloads: Option<u64>,
    #[serde(default)]
    likes: Option<u64>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    siblings: Option<Vec<HfSibling>>,
}

/// Search GGUF text-generation models by query string.
pub async fn search_gguf_models(
    hub_origin: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<SearchResult>> {
    fetch_models(hub_origin, Some(query.trim()), limit).await
}

/// Top GGUF text-generation models by download count (no search string).
pub async fn popular_gguf_models(hub_origin: &str, limit: u32) -> Result<Vec<SearchResult>> {
    fetch_models(hub_origin, None, limit).await
}

async fn fetch_models(
    hub_origin: &str,
    search: Option<&str>,
    limit: u32,
) -> Result<Vec<SearchResult>> {
    let base = hub_origin.trim_end_matches('/');
    let url = format!("{base}/api/models");

    let client = reqwest::Client::new();
    let mut req = client
        .get(&url)
        .query(&[
            ("pipeline_tag", "text-generation"),
            ("filter", "gguf"),
            ("sort", "downloads"),
            ("direction", "-1"),
            ("limit", &limit.to_string()),
            ("full", "true"),
        ]);

    if let Some(q) = search {
        if !q.is_empty() {
            req = req.query(&[("search", q)]);
        }
    }

    let response = req
        .send()
        .await
        .with_context(|| format!("failed to request {url}"))?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Hub search failed: HTTP {} for {}",
            response.status(),
            url
        );
    }

    let items: Vec<HfModelItem> = response
        .json()
        .await
        .with_context(|| format!("failed to parse JSON from {url}"))?;

    Ok(items.into_iter().map(into_search_result).collect())
}

fn into_search_result(item: HfModelItem) -> SearchResult {
    let gguf_files: Vec<GgufFileInfo> = item
        .siblings
        .unwrap_or_default()
        .into_iter()
        .filter(|s| {
            let n = s.rfilename.to_ascii_lowercase();
            n.ends_with(".gguf")
        })
        .map(|s| {
            let quantization_label = guess_quantization_from_filename(&s.rfilename);
            GgufFileInfo {
                filename: s.rfilename,
                size_bytes: s.size,
                quantization_label,
            }
        })
        .collect();

    SearchResult {
        repo_id: item.id,
        author: item.author,
        downloads: item.downloads.unwrap_or(0),
        likes: item.likes.unwrap_or(0),
        tags: item.tags,
        gguf_files,
    }
}

fn guess_quantization_from_filename(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".gguf")?;
    let last = stem.rsplit('-').next()?;
    if last.starts_with("IQ") || (last.starts_with('Q') && last.len() > 1) {
        let c1 = last.chars().nth(1);
        if last.starts_with("IQ") || c1.is_some_and(|c| c.is_ascii_digit()) {
            return Some(last.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantization_from_filename_trailing_segment() {
        assert_eq!(
            guess_quantization_from_filename("Llama-3.2-1B-Instruct-Q4_K_M.gguf").as_deref(),
            Some("Q4_K_M")
        );
        assert_eq!(
            guess_quantization_from_filename("gemma-2-2b-it-IQ3_M.gguf").as_deref(),
            Some("IQ3_M")
        );
        assert_eq!(guess_quantization_from_filename("weird.gguf"), None);
    }
}
