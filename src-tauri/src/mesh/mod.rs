//! Embedded mesh transport — Phase 7.
//!
//! Replaces the Phase-4 dependency on a system-installed `tailscale`
//! CLI with a bundled Go sidecar that links Tailscale's userspace
//! network stack (`tsnet`). Everything else in the app — chat, library,
//! orchestration, streaming — keeps using the same Rust APIs; we only
//! swap the transport floor.
//!
//! Layout:
//! - [`types`]           — wire types ([`MeshStatus`], [`MeshPorts`], …)
//! - [`control`]         — HTTP client for the sidecar's `/status` etc.
//! - [`sidecar`]         — Tauri sidecar spawn + lifecycle
//! - [`socks5`]          — minimal client used by chat outbound
//! - [`proxy_protocol`]  — PROXY v1 parser used by chat inbound

pub mod control;
pub mod proxy_protocol;
pub mod sidecar;
pub mod socks5;
pub mod types;

#[cfg(test)]
mod tests;
