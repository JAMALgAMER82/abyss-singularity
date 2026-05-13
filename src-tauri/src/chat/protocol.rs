//! Frame protocol: 4-byte big-endian length + UTF-8 JSON payload.
//!
//! Cap the frame size at 64 KiB — chat messages are tiny and the only
//! field that can balloon is `Chat.body`, which we additionally clamp at
//! the call site. The cap protects us from a malicious peer trying to
//! make us allocate gigabytes via a bogus length prefix.

use anyhow::{anyhow, Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::types::ChatProtocol;

pub const MAX_FRAME_BYTES: usize = 64 * 1024;

pub async fn write_frame<W: AsyncWriteExt + Unpin>(w: &mut W, msg: &ChatProtocol) -> Result<()> {
    let bytes = serde_json::to_vec(msg).context("serialising chat frame")?;
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(anyhow!("frame too large: {} bytes", bytes.len()));
    }
    w.write_u32(bytes.len() as u32).await.context("writing frame length")?;
    w.write_all(&bytes).await.context("writing frame body")?;
    w.flush().await.ok();
    Ok(())
}

pub async fn read_frame<R: AsyncReadExt + Unpin>(r: &mut R) -> Result<ChatProtocol> {
    let len = r.read_u32().await.context("reading frame length")? as usize;
    if len == 0 {
        return Err(anyhow!("empty frame"));
    }
    if len > MAX_FRAME_BYTES {
        return Err(anyhow!("frame too large: {len} bytes (max {MAX_FRAME_BYTES})"));
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).await.context("reading frame body")?;
    let msg: ChatProtocol = serde_json::from_slice(&buf).context("parsing chat frame")?;
    Ok(msg)
}

#[cfg(test)]
pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(not(test))]
pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
