//! Parser for PROXY protocol v1 (text form).
//!
//! Used by inbound chat sessions to learn the real peer IP that the
//! sidecar's forwarder writes as the first line of every conn.
//!
//! Format: `PROXY <fam> <src_ip> <dst_ip> <src_port> <dst_port>\r\n`
//! Fam is one of TCP4 / TCP6 / UNKNOWN. UNKNOWN means the proxy
//! couldn't determine — the rest of the line is implementation-defined.

use anyhow::{anyhow, Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::tcp::OwnedReadHalf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyV1Header {
    pub src_ip:   String,
    pub src_port: u16,
}

/// Read the PROXY v1 header from a stream and return what's left (a
/// `BufReader` containing any bytes after `\r\n`).
pub async fn read_header(
    reader: &mut BufReader<OwnedReadHalf>,
) -> Result<ProxyV1Header> {
    let mut line = String::new();
    let n = reader
        .read_line(&mut line)
        .await
        .context("reading PROXY v1 line")?;
    if n == 0 {
        return Err(anyhow!("connection closed before PROXY header"));
    }
    parse_line(line.trim_end_matches(['\r', '\n']))
}

pub fn parse_line(line: &str) -> Result<ProxyV1Header> {
    let mut parts = line.split_whitespace();
    let prefix = parts.next().ok_or_else(|| anyhow!("empty PROXY line"))?;
    if prefix != "PROXY" {
        return Err(anyhow!("expected 'PROXY ' prefix, got {prefix:?}"));
    }
    let fam = parts.next().ok_or_else(|| anyhow!("missing family"))?;
    match fam {
        "UNKNOWN" => Ok(ProxyV1Header { src_ip: "0.0.0.0".into(), src_port: 0 }),
        "TCP4" | "TCP6" => {
            let src_ip   = parts.next().ok_or_else(|| anyhow!("missing src ip"))?;
            let _dst_ip  = parts.next().ok_or_else(|| anyhow!("missing dst ip"))?;
            let src_port = parts.next().ok_or_else(|| anyhow!("missing src port"))?;
            let _dst_port= parts.next().ok_or_else(|| anyhow!("missing dst port"))?;
            Ok(ProxyV1Header {
                src_ip:   src_ip.to_string(),
                src_port: src_port.parse().context("parsing src port")?,
            })
        }
        other => Err(anyhow!("unknown PROXY family: {other}")),
    }
}
