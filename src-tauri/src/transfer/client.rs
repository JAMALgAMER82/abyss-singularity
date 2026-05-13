//! Outbound transfer sender. After the receiver returns `FileAccept`,
//! we open a SOCKS5 connection to `peer:TRANSFER_PORT`, write the
//! transfer-id + start-offset handshake, then stream the source file.
//!
//! Supports resumable sends: if the receiver requested `resume_offset > 0`
//! the sender seeks to that byte before streaming.
//!
//! Bandwidth-limited via a token bucket so a transfer doesn't saturate
//! the user's uplink while they're trying to game-stream on the same
//! pipe. Default 16 MB/s — adjustable per-call via [`send_options`].

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Runtime};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio::time::timeout;

use super::protocol::write_handshake;
use super::types::{Direction, TransferEvent, TRANSFER_EVENT, TRANSFER_PORT};
use crate::mesh::{socks5, types::MeshPorts};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Per-send tuning. Public so a future Settings UI can override.
#[derive(Debug, Clone, Copy)]
pub struct SendOptions {
    pub bandwidth_bytes_per_sec: u64,
    pub chunk_bytes:             usize,
}
impl Default for SendOptions {
    fn default() -> Self {
        Self {
            bandwidth_bytes_per_sec: 16 * 1024 * 1024,   // 16 MB/s default cap
            chunk_bytes:             64 * 1024,
        }
    }
}

pub async fn send_file<R: Runtime>(
    app:           AppHandle<R>,
    peer_addr:     String,
    transfer_id:   String,
    source:        &Path,
    file_size:     u64,
    start_offset:  u64,
) -> Result<()> {
    send_file_with(app, peer_addr, transfer_id, source, file_size, start_offset, SendOptions::default()).await
}

pub async fn send_file_with<R: Runtime>(
    app:           AppHandle<R>,
    peer_addr:     String,
    transfer_id:   String,
    source:        &Path,
    file_size:     u64,
    start_offset:  u64,
    opts:          SendOptions,
) -> Result<()> {
    let ports = MeshPorts::default();
    let mut stream = timeout(
        CONNECT_TIMEOUT,
        socks5::connect_through_socks5("127.0.0.1", ports.socks5, &peer_addr, TRANSFER_PORT),
    )
    .await
    .with_context(|| format!("SOCKS5 connect timeout to {peer_addr}:{TRANSFER_PORT}"))?
    .with_context(|| format!("SOCKS5 connect failed to {peer_addr}:{TRANSFER_PORT}"))?;

    write_handshake(&mut stream, &transfer_id, start_offset).await?;

    let _ = app.emit(TRANSFER_EVENT, TransferEvent::Started {
        transfer_id: transfer_id.clone(),
        direction:   Direction::Outgoing,
    });

    let mut file = File::open(source).await
        .with_context(|| format!("opening {}", source.display()))?;
    if start_offset > 0 {
        file.seek(SeekFrom::Start(start_offset)).await
            .with_context(|| format!("seek to {start_offset} in {}", source.display()))?;
    }

    let mut chunk = vec![0u8; opts.chunk_bytes];
    let mut bytes_done = start_offset;
    let started     = Instant::now();
    let mut last_emit = Instant::now();
    let mut bucket  = TokenBucket::new(opts.bandwidth_bytes_per_sec);

    loop {
        // Wait until the bucket has enough tokens for the next chunk.
        bucket.acquire(opts.chunk_bytes as u64).await;

        let n = file.read(&mut chunk).await.context("reading source file")?;
        if n == 0 { break; }
        stream.write_all(&chunk[..n]).await.context("writing chunk to peer")?;
        bytes_done += n as u64;
        if last_emit.elapsed().as_millis() >= 100 {
            let _ = app.emit(TRANSFER_EVENT, TransferEvent::Progress {
                transfer_id: transfer_id.clone(),
                bytes_done,
                bytes_total: file_size,
            });
            last_emit = Instant::now();
        }
    }
    stream.shutdown().await.ok();

    let _ = app.emit(TRANSFER_EVENT, TransferEvent::Completed {
        transfer_id: transfer_id.clone(),
        final_path:  None,
        sha256_ok:   true, // receiver verifies; we just report bytes-sent ok
    });
    log::info!("transfer: outbound {transfer_id} done in {}ms ({bytes_done} bytes, started at offset {start_offset})",
        started.elapsed().as_millis());
    Ok(())
}

/// Pure SHA-256 of a file. Used by the offer-construction step so the
/// receiver can verify what they download.
pub async fn sha256_of_file(path: &Path) -> Result<String> {
    let mut file  = File::open(path).await
        .with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut chunk  = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut chunk).await.context("reading for hash")?;
        if n == 0 { break; }
        hasher.update(&chunk[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

// ----------------------------------------------------------------------------
// Token bucket — simple, no dependencies. Refills lazily on `acquire`.
// ----------------------------------------------------------------------------

struct TokenBucket {
    capacity:        u64,
    refill_per_sec:  u64,
    available:       u64,
    last_refill:     Instant,
}

impl TokenBucket {
    fn new(bytes_per_sec: u64) -> Self {
        let capacity = bytes_per_sec.max(64 * 1024);
        Self {
            capacity,
            refill_per_sec: bytes_per_sec,
            available: capacity,
            last_refill: Instant::now(),
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let add = (elapsed * self.refill_per_sec as f64) as u64;
        if add > 0 {
            self.available = (self.available + add).min(self.capacity);
            self.last_refill = now;
        }
    }

    async fn acquire(&mut self, want: u64) {
        loop {
            self.refill();
            if self.available >= want {
                self.available -= want;
                return;
            }
            // Wait until enough has accrued — short sleep so the loop
            // remains responsive to large bursts.
            let deficit = want - self.available;
            let wait_secs = deficit as f64 / self.refill_per_sec as f64;
            tokio::time::sleep(Duration::from_secs_f64(wait_secs.min(0.1))).await;
        }
    }
}
