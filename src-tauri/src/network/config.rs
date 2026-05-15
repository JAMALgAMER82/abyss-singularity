//! Persisted networking config.
//!
//! Currently just the Tailscale auth-state plumbing the in-app
//! "invite a friend" flow needs. Stored in the same settings.json
//! the rest of Abyss uses.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

pub const NETWORK_CONFIG_KEY: &str = "network.config";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Reusable Tailscale auth key the user has generated in their tailnet
    /// admin console. When set, this is the key we hand out via invite
    /// codes so friends can paste-and-join without a browser dance.
    #[serde(default)]
    pub host_invite_authkey: Option<String>,
    /// Friendly name the user wants displayed on invite codes they hand
    /// out, falls back to OS hostname when None.
    #[serde(default)]
    pub host_display_name: Option<String>,
    /// The auth key we redeemed from an invite, if any. We pass this to
    /// the mesh sidecar on respawn so tsnet authenticates against the
    /// host's tailnet rather than asking us to sign in via browser.
    #[serde(default)]
    pub redeemed_authkey: Option<String>,
    /// Bookkeeping: who issued the invite we redeemed. Surfaced in the UI.
    #[serde(default)]
    pub redeemed_from: Option<String>,
}

pub fn load<R: Runtime>(app: &AppHandle<R>) -> Result<NetworkConfig> {
    let store = app
        .store(crate::library::config::STORE_FILE)
        .context("opening settings store")?;
    let cfg = match store.get(NETWORK_CONFIG_KEY) {
        Some(v) => serde_json::from_value(v).context("deserialising network config")?,
        None => NetworkConfig::default(),
    };
    Ok(cfg)
}

pub fn save<R: Runtime>(app: &AppHandle<R>, cfg: &NetworkConfig) -> Result<()> {
    let store = app
        .store(crate::library::config::STORE_FILE)
        .context("opening settings store")?;
    let v = serde_json::to_value(cfg).context("serialising network config")?;
    store.set(NETWORK_CONFIG_KEY, v);
    store.save().context("flushing settings store")?;
    Ok(())
}
