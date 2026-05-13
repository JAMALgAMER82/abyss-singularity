//! Phase 11 — gamepad detection + smart auto-configuration.
//!
//! Detection itself lives in the frontend (Web Gamepad API gives us live
//! button/axis state for free). The Rust side handles persistence:
//! generating emulator-specific config files so a freshly-plugged-in
//! controller just works without the user opening any settings menus.
//!
//! Currently emulator support: RetroArch (covers most retro systems via
//! libretro cores; one autoconfig file is enough for every libretro
//! emulated platform).

pub mod commands;
pub mod retroarch;
pub mod types;

#[cfg(test)]
mod tests;
