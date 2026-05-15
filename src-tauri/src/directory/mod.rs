//! Phase 14 — global directory client.
//!
//! Talks to the [abyss-directory Cloudflare Worker](../../../abyss-directory)
//! for the GameRanger-style "everyone running Abyss is visible" experience.
//!
//! Architectural placement:
//!   * The directory is the **social layer** — who's online, friend list,
//!     friend requests, DMs, global chat.
//!   * The Tailscale mesh remains the **transport layer** — once two
//!     users want to actually play together / transfer files / lobby up,
//!     they exchange a mesh invite via the directory and one redeems it.
//!   * These are decoupled. Two users can be "friends in the directory"
//!     forever without ever joining the same tailnet — they just chat.
//!
//! Identity is a client-generated UUID stored in [`config::DirectoryConfig`].
//! It persists across reinstalls (saved in `settings.json`) so users keep
//! their friend list when they upgrade Abyss.

pub mod client;
pub mod commands;
pub mod config;
pub mod heartbeat;
pub mod types;
