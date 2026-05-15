//! Streaming HTTP download with progress callbacks.
//!
//! Writes the body to a destination file, flushing as it goes so a
//! crash mid-download leaves at most a truncated `.part` file we can
//! discard on the next attempt.
//!
//! Also handles `gh-latest://owner/repo/asset-substring` pseudo-URLs:
//! these are resolved at install time by querying the GitHub releases
//! API for the *latest* release of `owner/repo` and picking the asset
//! whose name contains `asset-substring` (case-insensitive). This lets
//! manifests survive upstream release renames without code edits.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use futures::StreamExt;
use serde::Deserialize;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// Resolve a manifest URL to a concrete HTTPS URL, expanding any
/// `gh-latest://` pseudo-URL via the GitHub API. Passes regular URLs
/// through unchanged.
pub async fn resolve_url(url: &str) -> Result<String> {
    let Some(rest) = url.strip_prefix("gh-latest://") else {
        return Ok(url.to_string());
    };
    // Expected shape: `owner/repo/asset-substring`. The substring is
    // matched against the asset's `name` field (case-insensitive); the
    // first match wins.
    let mut parts = rest.splitn(3, '/');
    let owner   = parts.next().ok_or_else(|| anyhow!("gh-latest:// missing owner: {url}"))?;
    let repo    = parts.next().ok_or_else(|| anyhow!("gh-latest:// missing repo: {url}"))?;
    let needle  = parts.next().ok_or_else(|| anyhow!("gh-latest:// missing asset substring: {url}"))?;
    let needle  = needle.to_ascii_lowercase();

    #[derive(Deserialize)]
    struct Asset { name: String, browser_download_url: String }
    #[derive(Deserialize)]
    struct Release { assets: Vec<Asset> }

    let api_url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let client = reqwest::Client::builder()
        .user_agent("AbyssSingularity/0.1 installer")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("building reqwest client")?;
    let release: Release = client
        .get(&api_url)
        .header("Accept", "application/vnd.github+json")
        .send().await
        .with_context(|| format!("GET {api_url}"))?
        .error_for_status()
        .with_context(|| format!("GitHub API error for {owner}/{repo}"))?
        .json().await
        .context("parsing GitHub release JSON")?;

    let pick = release.assets
        .into_iter()
        .find(|a| a.name.to_ascii_lowercase().contains(&needle))
        .ok_or_else(|| anyhow!(
            "no asset matching {:?} in latest release of {owner}/{repo}",
            needle
        ))?;
    log::info!("installer: resolved {url} -> {}", pick.browser_download_url);
    Ok(pick.browser_download_url)
}

pub async fn fetch_to_file<F>(
    url:     &str,
    dest:    &Path,
    mut on_progress: F,
) -> Result<()>
where
    F: FnMut(u64, Option<u64>) + Send,
{
    let client = reqwest::Client::builder()
        .user_agent("AbyssSingularity/0.1 installer")
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .context("building reqwest client")?;

    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        return Err(anyhow!("HTTP {} from {url}", resp.status()));
    }
    let bytes_total = resp.content_length();
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }

    let mut file = File::create(dest).await
        .with_context(|| format!("create {}", dest.display()))?;
    let mut stream    = resp.bytes_stream();
    let mut bytes_done = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("reading body chunk")?;
        file.write_all(&chunk).await.context("write body chunk")?;
        bytes_done += chunk.len() as u64;
        on_progress(bytes_done, bytes_total);
    }
    file.flush().await.context("flush dest")?;
    Ok(())
}
