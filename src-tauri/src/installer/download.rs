//! Streaming HTTP download with progress callbacks.
//!
//! Writes the body to a destination file, flushing as it goes so a
//! crash mid-download leaves at most a truncated `.part` file we can
//! discard on the next attempt.

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use futures::StreamExt;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

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
