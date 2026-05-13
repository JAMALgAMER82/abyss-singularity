//! HTTP client for the sidecar's `127.0.0.1:CTL_PORT` control API.

use std::time::Duration;

use anyhow::{Context, Result};

use super::types::{MeshPorts, MeshStatus};

const HTTP_TIMEOUT: Duration = Duration::from_secs(5);

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .no_proxy() // never route control plane traffic through anything
        .build()
        .context("building reqwest client")
}

pub async fn status(ports: MeshPorts) -> Result<MeshStatus> {
    let url = format!("http://127.0.0.1:{}/status", ports.control);
    let resp = client()?
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;
    let status: MeshStatus = resp
        .error_for_status()
        .context("control api non-2xx")?
        .json()
        .await
        .context("parsing /status JSON")?;
    Ok(status)
}

#[allow(dead_code)] // exposed for future shutdown coordination
pub async fn shutdown(ports: MeshPorts) -> Result<()> {
    let url = format!("http://127.0.0.1:{}/shutdown", ports.control);
    let _ = client()?
        .post(&url)
        .send()
        .await
        .with_context(|| format!("POST {url}"))?;
    Ok(())
}

/// Cheap reachability probe — used by the supervisor to know when the
/// child is ready to serve requests.
#[allow(dead_code)] // wired into a later auth-flow polling loop
pub async fn health(ports: MeshPorts) -> bool {
    let url = format!("http://127.0.0.1:{}/health", ports.control);
    let Ok(c) = client() else { return false };
    matches!(
        c.get(&url).send().await,
        Ok(r) if r.status().is_success()
    )
}
