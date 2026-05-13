//! Inbound transfer receiver.
//!
//! Listens on 127.0.0.1:TRANSFER_PORT (the Go sidecar's forwarder
//! shovels tsnet inbound here with a PROXY v1 prefix). For each accepted
//! connection:
//!   1. parse PROXY v1
//!   2. read 4-byte tid_len + tid bytes
//!   3. look up the registered incoming offer by tid
//!   4. stream remaining bytes to the destination file, hashing as we go
//!   5. verify sha256, emit completion event

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use super::protocol::read_handshake;
use super::state::TransferState;
use super::types::{Direction, TransferEvent, TRANSFER_EVENT, TRANSFER_PORT};
use crate::mesh::proxy_protocol;

pub fn incoming_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| anyhow!("resolving app data dir: {e}"))?
        .join("incoming");
    std::fs::create_dir_all(&base)
        .with_context(|| format!("create incoming dir: {}", base.display()))?;
    Ok(base)
}

pub async fn run<R: Runtime>(app: AppHandle<R>, state: Arc<TransferState>) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", TRANSFER_PORT))
        .await
        .with_context(|| format!("binding transfer listener on 127.0.0.1:{TRANSFER_PORT}"))?;
    log::info!("transfer: listening on 127.0.0.1:{TRANSFER_PORT}");

    loop {
        let (stream, _addr) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                log::warn!("transfer: accept failed: {e}");
                continue;
            }
        };
        let app_c   = app.clone();
        let state_c = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_inbound(app_c, state_c, stream).await {
                log::warn!("transfer: inbound failed: {e:#}");
            }
        });
    }
}

async fn handle_inbound<R: Runtime>(
    app:    AppHandle<R>,
    state:  Arc<TransferState>,
    stream: tokio::net::TcpStream,
) -> Result<()> {
    let (rx, _tx) = stream.into_split();
    let mut buf = BufReader::new(rx);

    let _proxy = proxy_protocol::read_header(&mut buf).await
        .context("reading PROXY v1 header on transfer port")?;
    let (tid, start_offset) = read_handshake(&mut buf).await?;
    let offer = state
        .take_incoming(&tid)
        .ok_or_else(|| anyhow!("no registered transfer for tid {tid}"))?;

    let dir = incoming_dir(&app)?;
    let dest = dir.join(&offer.file_name);
    // Atomic-ish: write to .part then rename on success.
    let tmp = dest.with_extension(format!(
        "{}.part",
        dest.extension().and_then(|e| e.to_str()).unwrap_or("bin"),
    ));

    let _ = app.emit(TRANSFER_EVENT, TransferEvent::Started {
        transfer_id: tid.clone(),
        direction:   Direction::Incoming,
    });

    let started = Instant::now();
    // Open append-style if resuming; otherwise truncate.
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(start_offset == 0)
        .append(start_offset > 0)
        .open(&tmp)
        .await
        .with_context(|| format!("open {} (resume_at={start_offset})", tmp.display()))?;
    let mut hasher = Sha256::new();
    // If resuming, replay the existing bytes through the hasher so the
    // final digest matches the sender's hash of the *full* file.
    if start_offset > 0 {
        let mut prev = tokio::fs::File::open(&tmp).await
            .with_context(|| format!("re-open {} for hash backfill", tmp.display()))?;
        let mut replay_buf = vec![0u8; 64 * 1024];
        let mut consumed = 0u64;
        while consumed < start_offset {
            use tokio::io::AsyncReadExt as _;
            let n = prev.read(&mut replay_buf).await.context("backfill read")?;
            if n == 0 { break; }
            hasher.update(&replay_buf[..n]);
            consumed += n as u64;
        }
    }
    let mut chunk  = vec![0u8; 64 * 1024];
    let mut bytes_done = start_offset;
    let mut last_emit = Instant::now();

    loop {
        let n = buf.read(&mut chunk).await.context("reading transfer body")?;
        if n == 0 { break; }
        hasher.update(&chunk[..n]);
        file.write_all(&chunk[..n]).await.context("writing chunk")?;
        bytes_done += n as u64;

        // Throttle progress emits so we don't flood the event bus.
        if last_emit.elapsed().as_millis() >= 100 {
            let _ = app.emit(TRANSFER_EVENT, TransferEvent::Progress {
                transfer_id: tid.clone(),
                bytes_done,
                bytes_total: offer.file_size,
            });
            last_emit = Instant::now();
        }
    }
    file.flush().await.ok();
    drop(file);

    let actual = hex::encode(hasher.finalize());
    let sha256_ok = actual == offer.sha256;
    let final_path = if sha256_ok {
        // Games always land in the `incoming/` folder — no auto-place,
        // no save-routing, no race conditions. User opens incoming/,
        // adds it as a library scan path if they want.
        std::fs::rename(&tmp, &dest)
            .with_context(|| format!("rename {} -> {}", tmp.display(), dest.display()))?;
        Some(dest)
    } else {
        let _ = std::fs::remove_file(&tmp);
        None
    };

    let _ = app.emit(TRANSFER_EVENT, TransferEvent::Completed {
        transfer_id: tid.clone(),
        final_path,
        sha256_ok,
    });
    log::info!("transfer: inbound {tid} done in {}ms, sha256_ok={sha256_ok}",
        started.elapsed().as_millis());
    Ok(())
}
