//! `orchestration` — emulator launcher.
//!
//! Phase 3 of the Abyss roadmap. The user supplies a list of emulator
//! binaries + an optional argument template; we resolve a library entry to
//! its target emulator (via [`types::OrchestrationConfig::assignments`]),
//! expand `{game_path}`-style placeholders, then spawn the child with
//! piped stdout/stderr and emit per-line events back to the React UI.
//!
//! Module layout:
//! - [`types`]    — wire types (EmulatorEntry, OrchestrationConfig, LaunchEvent, …)
//! - [`recipes`]  — built-in default emulator recipes + arg template expansion
//! - [`config`]   — persisted config via tauri-plugin-store
//! - [`launcher`] — child-process registry + spawn-and-track loop
//! - [`commands`] — `#[tauri::command]`s exposed to the frontend

pub mod commands;
pub mod config;
pub mod launcher;
pub mod recipes;
pub mod types;

#[cfg(target_os = "windows")]
pub mod embed;

#[cfg(test)]
mod tests;
