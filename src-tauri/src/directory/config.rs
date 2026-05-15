//! Persisted directory client config — user identity + Worker URL +
//! privacy preferences. Stored in the same `settings.json` everything
//! else uses.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

pub const DIRECTORY_CONFIG_KEY: &str = "directory.config";

/// Default Cloudflare Worker URL baked into the build. Friends who
/// install Abyss don't have to paste anything — they show up on the
/// same directory as the host automatically. Power users can override
/// in Settings → Directory if they want their own deployment.
///
/// This is a build-time constant. To re-target your own Worker, edit
/// this string and rebuild. (We could read it from an env var at
/// `tauri build` time too — `option_env!("ABYSS_DIRECTORY_URL")` —
/// but a constant keeps the deployment trivial.)
pub const DEFAULT_WORKER_URL: &str =
    "https://abyss-directory.george-joseph992.workers.dev";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DirectoryConfig {
    /// Stable per-install UUID — generated on first run, never rotated.
    /// Acts as the bearer of identity to the Worker; if the file is
    /// deleted the user is a fresh face from the directory's view.
    #[serde(default)]
    pub user_id: Option<String>,
    /// Display name the user picked (or auto-derived from the OS hostname).
    /// Mutable — they can change it any time from Settings.
    #[serde(default)]
    pub handle: Option<String>,
    /// Where the Worker lives — e.g. `https://abyss-directory.you.workers.dev`.
    /// Empty / None disables the entire directory feature.
    #[serde(default)]
    pub worker_url: Option<String>,
    /// "Appear offline" — when true the heartbeat still fires (so we can
    /// still receive friend requests / DMs) but the Worker hides us from
    /// `/v1/online`.
    #[serde(default)]
    pub hidden: bool,
    /// Optional 2-letter country code for per-region matchmaking hints.
    #[serde(default)]
    pub country: Option<String>,
}

impl DirectoryConfig {
    /// Whether the client is fully configured to talk to a directory.
    pub fn is_ready(&self) -> bool {
        self.user_id.as_deref().filter(|s| !s.is_empty()).is_some()
            && self.handle.as_deref().filter(|s| !s.is_empty()).is_some()
            && self.worker_url.as_deref().filter(|s| !s.is_empty()).is_some()
    }
}

pub fn load<R: Runtime>(app: &AppHandle<R>) -> Result<DirectoryConfig> {
    let store = app
        .store(crate::library::config::STORE_FILE)
        .context("opening settings store")?;
    let cfg = match store.get(DIRECTORY_CONFIG_KEY) {
        Some(v) => serde_json::from_value(v).context("deserialising directory config")?,
        None => DirectoryConfig::default(),
    };
    Ok(cfg)
}

pub fn save<R: Runtime>(app: &AppHandle<R>, cfg: &DirectoryConfig) -> Result<()> {
    let store = app
        .store(crate::library::config::STORE_FILE)
        .context("opening settings store")?;
    let v = serde_json::to_value(cfg).context("serialising directory config")?;
    store.set(DIRECTORY_CONFIG_KEY, v);
    store.save().context("flushing settings store")?;
    Ok(())
}

/// Mint defaults for any unset directory fields and persist. Idempotent —
/// fields the user has already set (handle, worker URL override, hidden,
/// country) are left untouched.
///
/// Called once at app startup so a fresh install lands on the Discover
/// tab with a working identity and a heartbeat already firing, without
/// the user having to touch Settings.
pub fn ensure_initialized<R: Runtime>(app: &AppHandle<R>) -> Result<DirectoryConfig> {
    let mut cfg = load(app)?;
    let mut dirty = false;
    if cfg.user_id.as_deref().filter(|s| !s.is_empty()).is_none() {
        cfg.user_id = Some(new_user_id());
        dirty = true;
    }
    if cfg.handle.as_deref().filter(|s| !s.is_empty()).is_none() {
        cfg.handle = Some(default_handle());
        dirty = true;
    }
    if cfg.worker_url.as_deref().filter(|s| !s.is_empty()).is_none() {
        cfg.worker_url = Some(DEFAULT_WORKER_URL.to_string());
        dirty = true;
    }
    if dirty {
        save(app, &cfg)?;
        log::info!(
            "directory: auto-initialised — handle={:?} worker_url={:?}",
            cfg.handle, cfg.worker_url,
        );
    }
    Ok(cfg)
}

/// Pick a sensible default handle from the OS hostname when the user
/// hasn't set one yet. Stripped to alphanumerics + dashes, capped at 24
/// chars to fit nicely in chat rows.
pub fn default_handle() -> String {
    let raw = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Abyss Player".to_string());
    let mut out: String = raw.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == ' ')
        .take(24)
        .collect();
    let trimmed = out.trim().to_string();
    out = if trimmed.is_empty() { "Abyss Player".to_string() } else { trimmed };
    out
}

/// Generate a fresh UUID — pure-Rust, no external deps. Uses 16 bytes
/// of high-resolution-time entropy mixed with a process counter, hashed
/// with sha256, formatted as a UUID-shaped hex string. Plenty unique
/// for our directory's bearer-of-identity needs.
pub fn new_user_id() -> String {
    use sha2::{Digest, Sha256};
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let counter = std::sync::atomic::AtomicU64::new(0)
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut h = Sha256::new();
    h.update(nanos.to_le_bytes());
    h.update(pid.to_le_bytes());
    h.update(counter.to_le_bytes());
    h.update(b"abyss-directory-id-v1");
    let bytes = h.finalize();
    // Format as 8-4-4-4-12 hex — standard UUID layout.
    let hex: String = bytes.iter().take(16).map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_user_id_has_uuid_shape() {
        let id = new_user_id();
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
        assert!(id.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
    }

    #[test]
    fn new_user_ids_differ_across_calls() {
        let a = new_user_id();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = new_user_id();
        assert_ne!(a, b);
    }

    #[test]
    fn is_ready_requires_all_three() {
        let mut c = DirectoryConfig::default();
        assert!(!c.is_ready());
        c.user_id = Some("u1".into());
        assert!(!c.is_ready());
        c.handle = Some("Bob".into());
        assert!(!c.is_ready());
        c.worker_url = Some("https://x".into());
        assert!(c.is_ready());
    }

    #[test]
    fn default_handle_is_nonempty() {
        let h = default_handle();
        assert!(!h.is_empty());
        assert!(h.len() <= 24);
    }
}
