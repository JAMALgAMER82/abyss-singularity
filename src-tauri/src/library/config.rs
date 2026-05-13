//! Persisted library configuration backed by `tauri-plugin-store`.
//!
//! Holds the user's selected scan paths and (optionally, for Phase 2.3)
//! their IGDB credentials. Centralised so commands don't have to know
//! about the underlying store handle.

use anyhow::{Context, Result};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

use super::types::LibraryConfig;

/// File the store writes to under the app data dir.
pub const STORE_FILE: &str = "settings.json";
/// Key under which the [`LibraryConfig`] blob lives.
pub const LIBRARY_CONFIG_KEY: &str = "library.config";

pub fn load<R: Runtime>(app: &AppHandle<R>) -> Result<LibraryConfig> {
    let store = app
        .store(STORE_FILE)
        .context("opening settings store")?;
    let value = store.get(LIBRARY_CONFIG_KEY);
    let cfg = match value {
        Some(v) => serde_json::from_value::<LibraryConfig>(v)
            .context("deserialising library config")?,
        None => LibraryConfig::default(),
    };
    Ok(cfg)
}

pub fn save<R: Runtime>(app: &AppHandle<R>, cfg: &LibraryConfig) -> Result<()> {
    let store = app
        .store(STORE_FILE)
        .context("opening settings store")?;
    let value = serde_json::to_value(cfg).context("serialising library config")?;
    store.set(LIBRARY_CONFIG_KEY, value);
    store.save().context("flushing settings store")?;
    Ok(())
}
