//! Persisted orchestration config (emulator definitions + per-platform
//! assignments) backed by `tauri-plugin-store`.

use anyhow::{Context, Result};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

use super::types::OrchestrationConfig;

/// Stored in the same `settings.json` file the library config uses, just
/// under its own key.
pub const ORCHESTRATION_CONFIG_KEY: &str = "orchestration.config";

pub fn load<R: Runtime>(app: &AppHandle<R>) -> Result<OrchestrationConfig> {
    let store = app
        .store(crate::library::config::STORE_FILE)
        .context("opening settings store")?;
    let cfg = match store.get(ORCHESTRATION_CONFIG_KEY) {
        Some(v) => serde_json::from_value(v).context("deserialising orchestration config")?,
        None => OrchestrationConfig::default(),
    };
    Ok(cfg)
}

pub fn save<R: Runtime>(app: &AppHandle<R>, cfg: &OrchestrationConfig) -> Result<()> {
    let store = app
        .store(crate::library::config::STORE_FILE)
        .context("opening settings store")?;
    let v = serde_json::to_value(cfg).context("serialising orchestration config")?;
    store.set(ORCHESTRATION_CONFIG_KEY, v);
    store.save().context("flushing settings store")?;
    Ok(())
}
