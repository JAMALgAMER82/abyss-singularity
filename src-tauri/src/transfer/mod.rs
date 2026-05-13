//! Phase 9 — peer-to-peer game transfer over the embedded mesh.
//!
//! Reuses everything from Phase 7: the Go sidecar forwards tsnet:47993
//! → 127.0.0.1:47993 with PROXY v1, and outbound dials go through the
//! same SOCKS5 proxy at 127.0.0.1:1080. The only new wire format is the
//! transfer-id header that prefixes the file body so the receiver can
//! correlate the byte stream with the FileOffer that introduced it.

pub mod client;
pub mod commands;
pub mod protocol;
pub mod server;
pub mod state;
pub mod types;

#[cfg(test)]
mod tests;
