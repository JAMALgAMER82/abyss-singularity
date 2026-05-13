//! `library` — directory scanner, JSON cache, and IGDB enrichment.
//!
//! Module layout:
//! - [`types`]      — wire types (LibraryEntry, Platform, ScanReport, …)
//! - [`platforms`]  — extension → Platform mapping
//! - [`scanner`]    — walkdir-based fs scan
//! - [`cache`]      — load/save library.json
//! - [`config`]     — persisted [`LibraryConfig`] via tauri-plugin-store
//! - [`commands`]   — `#[tauri::command]`s exposed to the React side
//! - `igdb`         — Phase 2.3 (not yet present)

pub mod cache;
pub mod commands;
pub mod config;
pub mod igdb;
pub mod platforms;
pub mod scanner;
pub mod types;

#[cfg(test)]
mod tests;
