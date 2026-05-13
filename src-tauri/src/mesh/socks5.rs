//! Minimal SOCKS5 CONNECT client.
//!
//! Talks to the sidecar's local SOCKS5 outbound proxy
//! (127.0.0.1:SOCKS_PORT) so that any TCP connection we route through
//! `connect_through_socks5` reaches tailnet peers via the embedded
//! tsnet stack.
//!
//! Implementing this inline (~60 lines) keeps us off another tokio-socks
//! crate dependency.

use anyhow::{anyhow, Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const VER: u8 = 0x05;
const CMD_CONNECT: u8 = 0x01;
const RSV: u8 = 0x00;
const ATYP_IPV4: u8   = 0x01;
const ATYP_DOMAIN: u8 = 0x03;
const ATYP_IPV6: u8   = 0x04;
const NO_AUTH: u8 = 0x00;

/// Connect to `target_host:target_port` *through* the SOCKS5 proxy at
/// `proxy_host:proxy_port`. `target_host` can be an IPv4 dotted-quad,
/// an IPv6 literal, or a DNS name.
pub async fn connect_through_socks5(
    proxy_host:  &str,
    proxy_port:  u16,
    target_host: &str,
    target_port: u16,
) -> Result<TcpStream> {
    let mut s = TcpStream::connect((proxy_host, proxy_port))
        .await
        .with_context(|| format!("connect to SOCKS5 {proxy_host}:{proxy_port}"))?;

    // Greeting: VER, NMETHODS, METHODS[0]=NO_AUTH
    s.write_all(&[VER, 0x01, NO_AUTH]).await.context("write greeting")?;
    let mut greet = [0u8; 2];
    s.read_exact(&mut greet).await.context("read greeting reply")?;
    if greet[0] != VER || greet[1] != NO_AUTH {
        return Err(anyhow!("SOCKS5 greeting refused: {greet:?}"));
    }

    // CONNECT request: VER, CMD, RSV, ATYP, addr, port
    let mut req = Vec::with_capacity(8 + target_host.len());
    req.extend_from_slice(&[VER, CMD_CONNECT, RSV]);
    if let Ok(ip) = target_host.parse::<std::net::Ipv4Addr>() {
        req.push(ATYP_IPV4);
        req.extend_from_slice(&ip.octets());
    } else if let Ok(ip) = target_host.parse::<std::net::Ipv6Addr>() {
        req.push(ATYP_IPV6);
        req.extend_from_slice(&ip.octets());
    } else {
        if target_host.len() > 255 {
            return Err(anyhow!("SOCKS5 hostname too long: {} bytes", target_host.len()));
        }
        req.push(ATYP_DOMAIN);
        req.push(target_host.len() as u8);
        req.extend_from_slice(target_host.as_bytes());
    }
    req.extend_from_slice(&target_port.to_be_bytes());
    s.write_all(&req).await.context("write CONNECT")?;

    // Reply: VER, REP, RSV, ATYP, bound-addr, bound-port
    let mut head = [0u8; 4];
    s.read_exact(&mut head).await.context("read CONNECT reply head")?;
    if head[0] != VER {
        return Err(anyhow!("SOCKS5 reply: bad version {}", head[0]));
    }
    if head[1] != 0x00 {
        return Err(anyhow!("SOCKS5 CONNECT failed: REP={}", head[1]));
    }
    // Consume the bound-addr + port so the stream is clean for the caller.
    match head[3] {
        ATYP_IPV4 => {
            let mut b = [0u8; 4 + 2]; s.read_exact(&mut b).await.context("read bound addr v4")?;
        }
        ATYP_IPV6 => {
            let mut b = [0u8; 16 + 2]; s.read_exact(&mut b).await.context("read bound addr v6")?;
        }
        ATYP_DOMAIN => {
            let mut len = [0u8; 1]; s.read_exact(&mut len).await.context("read bound domain len")?;
            let mut b = vec![0u8; len[0] as usize + 2]; s.read_exact(&mut b).await.context("read bound domain")?;
        }
        other => return Err(anyhow!("SOCKS5 reply: unknown ATYP {other}")),
    }
    Ok(s)
}
