//! Wire protocol on the dedicated transfer TCP stream:
//!
//!   [4-byte BE tid_len][tid bytes][8-byte BE start_offset][raw file bytes...]
//!
//! `start_offset` is the byte position within the source file where the
//! sender is starting. 0 = full transfer. Used by resumable transfers
//! when the receiver already has a partial `.part` file.
//!
//! Bounded at 256 bytes for the transfer_id so a malformed sender can't
//! make the receiver allocate forever.

use anyhow::{anyhow, Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const MAX_TID_LEN: u32 = 256;

pub async fn write_handshake<W: AsyncWriteExt + Unpin>(
    w:           &mut W,
    transfer_id: &str,
    start_offset: u64,
) -> Result<()> {
    let bytes = transfer_id.as_bytes();
    if bytes.len() > MAX_TID_LEN as usize {
        return Err(anyhow!("transfer_id too long: {} bytes", bytes.len()));
    }
    w.write_u32(bytes.len() as u32).await.context("write tid_len")?;
    w.write_all(bytes).await.context("write tid")?;
    w.write_u64(start_offset).await.context("write start_offset")?;
    Ok(())
}

pub async fn read_handshake<R: AsyncReadExt + Unpin>(r: &mut R) -> Result<(String, u64)> {
    let len = r.read_u32().await.context("read tid_len")?;
    if len == 0 {
        return Err(anyhow!("empty transfer_id"));
    }
    if len > MAX_TID_LEN {
        return Err(anyhow!("transfer_id too long: {len} bytes"));
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf).await.context("read tid bytes")?;
    let tid = String::from_utf8(buf).context("transfer_id not valid utf-8")?;
    let start_offset = r.read_u64().await.context("read start_offset")?;
    Ok((tid, start_offset))
}

// Kept for backward compat with the tests that exercise just the tid
// half of the handshake. New code uses `write_handshake` / `read_handshake`.
#[cfg(test)]
pub async fn write_transfer_id<W: AsyncWriteExt + Unpin>(w: &mut W, transfer_id: &str) -> Result<()> {
    write_handshake(w, transfer_id, 0).await
}
#[cfg(test)]
pub async fn read_transfer_id<R: AsyncReadExt + Unpin>(r: &mut R) -> Result<String> {
    read_handshake(r).await.map(|(tid, _)| tid)
}
