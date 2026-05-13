//! TCP listener that accepts inbound chat connections.
//!
//! Binds on **127.0.0.1** only — Phase 7 introduced the mesh sidecar
//! which terminates the tsnet listener at the OS network boundary and
//! forwards each connection locally with a PROXY v1 line prefixed (see
//! [`crate::mesh::proxy_protocol`]). The local socket therefore needs
//! no public exposure.

use std::sync::Arc;

use anyhow::{Context, Result};
use tauri::{AppHandle, Runtime};
use tokio::io::BufReader;
use tokio::net::TcpListener;

use super::session;
use super::state::ChatState;
use crate::mesh::proxy_protocol;

pub async fn run<R: Runtime>(
    app:   AppHandle<R>,
    state: Arc<ChatState>,
    port:  u16,
) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .with_context(|| format!("binding chat listener to 127.0.0.1:{port}"))?;
    log::info!("chat: listening on 127.0.0.1:{port} (mesh-fronted)");

    loop {
        let (stream, _addr) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                log::warn!("chat: accept failed: {e}");
                continue;
            }
        };
        let app_c = app.clone();
        let state_c = state.clone();
        tokio::spawn(async move {
            // Peer's real IP is in the PROXY v1 header the sidecar prepends.
            let (rx, tx) = stream.into_split();
            let mut buf  = BufReader::new(rx);
            let header   = match proxy_protocol::read_header(&mut buf).await {
                Ok(h) => h,
                Err(e) => {
                    log::warn!("chat: bad PROXY v1 header: {e:#}");
                    return;
                }
            };
            let peer_label = header.src_ip.clone();
            log::info!("chat: inbound from {peer_label}");
            session::run_split(app_c, state_c, buf, tx, peer_label, false).await;
        });
    }
}
