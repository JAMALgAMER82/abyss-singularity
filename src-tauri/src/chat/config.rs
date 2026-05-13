use anyhow::{Context, Result};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

use super::types::ChatConfig;

pub const CHAT_CONFIG_KEY: &str = "chat.config";

pub fn load<R: Runtime>(app: &AppHandle<R>) -> Result<ChatConfig> {
    let store = app
        .store(crate::library::config::STORE_FILE)
        .context("opening settings store")?;
    let cfg = match store.get(CHAT_CONFIG_KEY) {
        Some(v) => serde_json::from_value(v).context("deserialising chat config")?,
        None => ChatConfig::default(),
    };
    Ok(cfg)
}

pub fn save<R: Runtime>(app: &AppHandle<R>, cfg: &ChatConfig) -> Result<()> {
    let store = app
        .store(crate::library::config::STORE_FILE)
        .context("opening settings store")?;
    let v = serde_json::to_value(cfg).context("serialising chat config")?;
    store.set(CHAT_CONFIG_KEY, v);
    store.save().context("flushing settings store")?;
    Ok(())
}
