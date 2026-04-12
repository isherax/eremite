use std::path::Path;

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

const DEFAULT_BASE_URL: &str = "https://huggingface.co";

pub fn build_download_url(base_url: &str, repo_id: &str, filename: &str) -> String {
    format!("{}/{}/resolve/main/{}", base_url, repo_id, filename)
}

pub fn default_base_url() -> &'static str {
    DEFAULT_BASE_URL
}

pub struct DownloadResult {
    pub size_bytes: u64,
    pub sha256: String,
}

/// Downloads a file from the given URL, streams it to `dest_path`, and computes
/// a SHA-256 digest as it writes. Returns the total bytes written and the hex digest.
///
/// `on_progress` is called with (bytes_so_far, total_bytes_option) after each chunk.
pub async fn download_file(
    url: &str,
    dest_path: &Path,
    on_progress: impl Fn(u64, Option<u64>),
) -> Result<DownloadResult> {
    if let Some(parent) = dest_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let response = reqwest::get(url)
        .await
        .with_context(|| format!("failed to request {}", url))?;

    if !response.status().is_success() {
        bail!(
            "download failed: HTTP {} for {}",
            response.status(),
            url
        );
    }

    let total_size = response.content_length();
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(dest_path)
        .await
        .with_context(|| format!("failed to create file {}", dest_path.display()))?;

    let mut hasher = Sha256::new();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("error reading download stream")?;
        file.write_all(&chunk)
            .await
            .context("error writing to file")?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;
        on_progress(downloaded, total_size);
    }

    file.flush().await.context("error flushing file")?;

    let sha256 = format!("{:x}", hasher.finalize());

    Ok(DownloadResult {
        size_bytes: downloaded,
        sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_construction_standard() {
        let url = build_download_url(
            "https://huggingface.co",
            "bartowski/Llama-3.2-1B-Instruct-GGUF",
            "Llama-3.2-1B-Instruct-Q4_K_M.gguf",
        );
        assert_eq!(
            url,
            "https://huggingface.co/bartowski/Llama-3.2-1B-Instruct-GGUF/resolve/main/Llama-3.2-1B-Instruct-Q4_K_M.gguf"
        );
    }

    #[test]
    fn url_construction_custom_base() {
        let url = build_download_url(
            "http://localhost:8080",
            "org/model-GGUF",
            "model-Q8_0.gguf",
        );
        assert_eq!(
            url,
            "http://localhost:8080/org/model-GGUF/resolve/main/model-Q8_0.gguf"
        );
    }

    #[test]
    fn sha256_known_value() {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(b"hello world");
        let result = format!("{:x}", hasher.finalize());
        assert_eq!(
            result,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }
}
