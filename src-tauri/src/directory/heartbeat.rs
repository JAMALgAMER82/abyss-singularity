//! Background heartbeat task — fires `POST /v1/presence` every 5 minutes
//! to keep us on the online list, and polls the inbox for new friend
//! requests / DMs / response acceptances. The 5-minute cadence fits
//! comfortably inside Cloudflare's free tier (~288 writes/user/day → a
//! 350-user friend group still stays under the 100k writes/day cap).
//!
//! Spawned once at app startup via [`spawn`]; gracefully no-ops when
//! the user hasn't configured the worker_url yet.

use std::time::Duration;

use tauri::{AppHandle, Emitter, Runtime};

use super::client::Directory;
use super::config;
use super::types::{
    DIR_DM_EVENT, DIR_FRIEND_REQUEST_EVENT, DIR_FRIEND_RESPONSE_EVENT,
};

pub const HEARTBEAT_INTERVAL_SECS: u64 = 5 * 60;
pub const INBOX_POLL_INTERVAL_SECS: u64 =     30;

/// Spawn the long-running heartbeat + inbox-poll task. Returns immediately;
/// the task lives until the app exits.
pub fn spawn<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        // Tick at the faster (inbox) cadence; heartbeat fires every
        // Nth tick where N = HEARTBEAT_INTERVAL / INBOX_POLL_INTERVAL.
        let mut tick = 0u64;
        let inbox_tick = HEARTBEAT_INTERVAL_SECS / INBOX_POLL_INTERVAL_SECS;
        loop {
            tokio::time::sleep(Duration::from_secs(INBOX_POLL_INTERVAL_SECS)).await;
            let cfg = match config::load(&app) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("directory: load config failed: {e:#}");
                    continue;
                }
            };
            if !cfg.is_ready() {
                // User hasn't set a worker_url yet — silently idle.
                continue;
            }
            let worker = cfg.worker_url.unwrap_or_default();
            let me     = cfg.user_id.unwrap_or_default();
            let handle = cfg.handle.unwrap_or_default();
            let client = match Directory::new(&worker, &me) {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("directory: client init failed: {e:#}");
                    continue;
                }
            };

            // Heartbeat every Nth tick (rough 5-minute cadence).
            if tick % inbox_tick == 0 {
                if let Err(e) = client.presence(
                    &handle,
                    env!("CARGO_PKG_VERSION"),
                    cfg.country.as_deref(),
                ).await {
                    log::warn!("directory: presence heartbeat failed: {e:#}");
                }
                // Echo the hidden flag — cheap and keeps the server in
                // sync if the user toggled it offline.
                if let Err(e) = client.set_hidden(cfg.hidden).await {
                    log::debug!("directory: set_hidden failed (non-fatal): {e:#}");
                }
            }

            // Inbox polling — friend requests, response acceptances, DMs.
            match client.friend_requests().await {
                Ok(rs) if !rs.is_empty() => {
                    let _ = app.emit(DIR_FRIEND_REQUEST_EVENT, rs);
                }
                Ok(_)  => {}
                Err(e) => log::debug!("directory: friend_requests poll: {e:#}"),
            }
            match client.friend_responses().await {
                Ok(rs) if !rs.is_empty() => {
                    let _ = app.emit(DIR_FRIEND_RESPONSE_EVENT, rs);
                }
                Ok(_)  => {}
                Err(e) => log::debug!("directory: friend_responses poll: {e:#}"),
            }
            match client.get_dms(60_000).await {
                Ok(rs) if !rs.is_empty() => {
                    let _ = app.emit(DIR_DM_EVENT, rs);
                }
                Ok(_)  => {}
                Err(e) => log::debug!("directory: dm poll: {e:#}"),
            }

            tick = tick.wrapping_add(1);
        }
    });
}
