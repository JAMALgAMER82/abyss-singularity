//! `network` — Phase 4 of the Abyss roadmap.
//!
//! Two coordinated pieces:
//!  * [`tailscale`] — wraps the user-installed `tailscale` CLI to surface
//!    daemon status + peer list to the UI.
//!  * [`latency`] + [`regions`] — TCP-connect probe of public regional
//!    endpoints, with a minimax recommender for picking a relay region
//!    that's fair to both players in a cross-continental co-op session.

pub mod commands;
pub mod config;
pub mod invite;
pub mod latency;
pub mod regions;
pub mod tailscale;
pub mod types;

#[cfg(test)]
mod tests;
