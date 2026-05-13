//! Persisted streaming config — shares the `settings.json` store with the
//! library + orchestration configs, just under its own key.

use anyhow::{Context, Result};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

use super::types::StreamingConfig;

pub const STREAMING_CONFIG_KEY: &str = "streaming.config";

pub fn load<R: Runtime>(app: &AppHandle<R>) -> Result<StreamingConfig> {
    let store = app
        .store(crate::library::config::STORE_FILE)
        .context("opening settings store")?;
    let cfg = match store.get(STREAMING_CONFIG_KEY) {
        Some(v) => serde_json::from_value(v).context("deserialising streaming config")?,
        None => StreamingConfig::default(),
    };
    Ok(cfg)
}

pub fn save<R: Runtime>(app: &AppHandle<R>, cfg: &StreamingConfig) -> Result<()> {
    let store = app
        .store(crate::library::config::STORE_FILE)
        .context("opening settings store")?;
    let v = serde_json::to_value(cfg).context("serialising streaming config")?;
    store.set(STREAMING_CONFIG_KEY, v);
    store.save().context("flushing settings store")?;
    Ok(())
}
