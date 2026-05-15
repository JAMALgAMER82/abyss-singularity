//! One-button "diagnose and repair" — runs every self-heal Abyss
//! already knows how to do, in order, and reports per-step status
//! back to the UI as a typed list.
//!
//! Designed for the "I gave a friend the installer and the wizard
//! shows red text" scenario: they hit a single button and Abyss
//! sorts itself out, OR tells them exactly which manual step is
//! left (BIOS dump, AV exception, etc.).

pub mod commands;
pub mod types;
