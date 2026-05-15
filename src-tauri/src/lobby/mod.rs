//! Phase 12 — in-app GameRanger-style lobby.
//!
//! A "room" is a small piece of shared state: one host, N members, one
//! game everyone agrees to play. The host advertises the room over the
//! existing chat layer ([`crate::chat::types::ChatProtocol::LobbyAdvertise`])
//! so every connected chat peer sees it without us standing up a second
//! listener. When the host hits **Start**, this module broadcasts a
//! [`crate::chat::types::ChatProtocol::LobbyStartGame`] frame and every
//! member's Abyss launches their local copy of the same game configured
//! as a netplay client pointing at the host's tailnet IP.
//!
//! Netplay support is RetroArch-only for now (the only emulator we ship
//! with first-class `-H` / `--connect=` CLI flags). Members who don't
//! have the game locally surface an actionable error in the UI.

pub mod commands;
pub mod handlers;
pub mod state;
pub mod types;
