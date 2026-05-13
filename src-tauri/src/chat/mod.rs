//! Phase 6.x — chat + presence over the Tailscale mesh.
//!
//! No third-party signalling. Each instance binds a TCP listener on a
//! user-configured port and (optionally) initiates outbound connections
//! to known mesh peers. Frames are length-prefixed JSON; messages are
//! [`types::ChatProtocol`] variants.
//!
//! State is held in a process-global [`state::ChatState`] (`OnceLock`)
//! rather than threaded through Tauri-managed state — this keeps the
//! command signatures clean and lets the server task reach the state
//! without being passed an `Arc` from each spawn point.

pub mod client;
pub mod commands;
pub mod config;
pub mod protocol;
pub mod server;
pub mod session;
pub mod state;
pub mod types;

#[cfg(test)]
mod tests;
