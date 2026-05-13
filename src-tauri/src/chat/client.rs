//! Outbound client — dials the peer through the sidecar's SOCKS5 proxy
//! so the connection traverses the embedded tsnet stack.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tauri::{AppHandle, Runtime};
use tokio::time::timeout;

use super::session;
use super::state::ChatState;
use crate::mesh::{socks5, types::MeshPorts};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);

pub async fn connect_and_run<R: Runtime>(
    app:   AppHandle<R>,
    state: Arc<ChatState>,
    host:  String,
    port:  u16,
) -> Result<()> {
    let ports = MeshPorts::default();
    let stream = timeout(
        CONNECT_TIMEOUT,
        socks5::connect_through_socks5("127.0.0.1", ports.socks5, &host, port),
    )
    .await
    .with_context(|| format!("connect timeout to {host}:{port} via SOCKS5"))?
    .with_context(|| format!("SOCKS5 connect failed: {host}:{port}"))?;

    let peer_label = host;
    tokio::spawn(async move {
        session::run(app, state, stream, peer_label, true).await;
    });
    Ok(())
}
