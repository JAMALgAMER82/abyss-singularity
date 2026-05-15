//! Phase 8 — emulator auto-installer.
//!
//! Pure-data manifest list (see [`manifests::all`]) defines what we know
//! how to install. The runtime pipeline is: HTTP-stream-download →
//! zip/7z extract → splice into `OrchestrationConfig` → optional auto-
//! assign-to-platform. Progress events flow through
//! [`types::INSTALL_PROGRESS_EVENT`].

pub mod bios_finder;
pub mod commands;
pub mod controller_setup;
pub mod download;
pub mod extract;
pub mod manifests;
pub mod streaming_apps;
pub mod types;

#[cfg(test)]
mod tests;
