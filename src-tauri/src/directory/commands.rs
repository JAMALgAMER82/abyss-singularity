//! Tauri commands exposed to the React frontend.

use tauri::{AppHandle, Runtime};

use super::client::Directory;
use super::config::{self, DirectoryConfig};
use super::types::{
    DirectMessage, Friend, FriendRequest, FriendResponse, GlobalChatMessage, OnlineUser,
};

/// Resolve a configured client or surface a clean error pointing the user
/// at the Settings panel they need to fill in. Used by every other
/// directory command — keeps the "you haven't set this up yet" message
/// consistent.
fn require_client<R: Runtime>(app: &AppHandle<R>) -> Result<Directory, String> {
    let cfg = config::load(app).map_err(|e| format!("{e:#}"))?;
    if !cfg.is_ready() {
        return Err(
            "Directory isn't set up yet. Go to Settings → Directory and \
             paste your Cloudflare Worker URL, then pick a display name."
                .to_string()
        );
    }
    let url = cfg.worker_url.unwrap_or_default();
    let me  = cfg.user_id.unwrap_or_default();
    Directory::new(&url, &me).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn dir_get_config<R: Runtime>(app: AppHandle<R>) -> Result<DirectoryConfig, String> {
    config::load(&app).map_err(|e| format!("{e:#}"))
}

/// Save (and lazily initialise) the directory config. If no `user_id`
/// is present we mint one now so the first call to any directory
/// command after this works without an extra round trip.
#[tauri::command]
pub fn dir_set_config<R: Runtime>(
    app:           AppHandle<R>,
    handle:        Option<String>,
    worker_url:    Option<String>,
    hidden:        Option<bool>,
    country:       Option<String>,
) -> Result<DirectoryConfig, String> {
    let mut cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    if cfg.user_id.is_none() || cfg.user_id.as_deref() == Some("") {
        cfg.user_id = Some(config::new_user_id());
    }
    if let Some(h) = handle {
        let h = h.trim().to_string();
        cfg.handle = (!h.is_empty()).then_some(h);
    }
    if cfg.handle.is_none() {
        cfg.handle = Some(config::default_handle());
    }
    if let Some(u) = worker_url {
        let u = u.trim().trim_end_matches('/').to_string();
        cfg.worker_url = (!u.is_empty()).then_some(u);
    }
    if let Some(h) = hidden   { cfg.hidden  = h; }
    if let Some(c) = country  {
        let c = c.trim().to_string();
        cfg.country = (!c.is_empty()).then_some(c);
    }
    config::save(&app, &cfg).map_err(|e| format!("{e:#}"))?;
    Ok(cfg)
}

#[tauri::command]
pub async fn dir_online<R: Runtime>(
    app:      AppHandle<R>,
    since_ms: Option<u64>,
) -> Result<Vec<OnlineUser>, String> {
    let c = require_client(&app)?;
    c.online(since_ms.unwrap_or(300_000)).await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn dir_send_friend_request<R: Runtime>(
    app:         AppHandle<R>,
    to_id:       String,
    invite_code: Option<String>,
    message:     Option<String>,
) -> Result<i64, String> {
    let c   = require_client(&app)?;
    let cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    let handle = cfg.handle.unwrap_or_else(|| "Abyss Player".to_string());
    c.send_friend_request(
        &to_id,
        &handle,
        invite_code.as_deref(),
        message.as_deref(),
    )
    .await
    .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn dir_friend_requests<R: Runtime>(app: AppHandle<R>) -> Result<Vec<FriendRequest>, String> {
    let c = require_client(&app)?;
    c.friend_requests().await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn dir_friend_responses<R: Runtime>(app: AppHandle<R>) -> Result<Vec<FriendResponse>, String> {
    let c = require_client(&app)?;
    c.friend_responses().await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn dir_accept_request<R: Runtime>(
    app:         AppHandle<R>,
    request_id:  i64,
    invite_code: Option<String>,
) -> Result<(), String> {
    let c = require_client(&app)?;
    c.accept_request(request_id, invite_code.as_deref())
        .await
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn dir_reject_request<R: Runtime>(
    app:        AppHandle<R>,
    request_id: i64,
) -> Result<(), String> {
    let c = require_client(&app)?;
    c.reject_request(request_id).await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn dir_friends<R: Runtime>(app: AppHandle<R>) -> Result<Vec<Friend>, String> {
    let c = require_client(&app)?;
    c.friends().await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn dir_send_dm<R: Runtime>(
    app:   AppHandle<R>,
    to_id: String,
    body:  String,
) -> Result<i64, String> {
    let c = require_client(&app)?;
    c.send_dm(&to_id, &body).await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn dir_get_dms<R: Runtime>(
    app:      AppHandle<R>,
    since_ms: Option<u64>,
) -> Result<Vec<DirectMessage>, String> {
    let c = require_client(&app)?;
    c.get_dms(since_ms.unwrap_or(86_400_000)).await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn dir_send_global_chat<R: Runtime>(
    app:  AppHandle<R>,
    body: String,
) -> Result<i64, String> {
    let c   = require_client(&app)?;
    let cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    let handle = cfg.handle.unwrap_or_else(|| "Abyss Player".to_string());
    c.send_global_chat(&handle, &body).await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn dir_get_global_chat<R: Runtime>(
    app:      AppHandle<R>,
    since_ms: Option<u64>,
) -> Result<Vec<GlobalChatMessage>, String> {
    let c = require_client(&app)?;
    c.get_global_chat(since_ms.unwrap_or(3_600_000)).await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn dir_block<R: Runtime>(app: AppHandle<R>, target_id: String) -> Result<(), String> {
    let c = require_client(&app)?;
    c.block(&target_id).await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn dir_unblock<R: Runtime>(app: AppHandle<R>, target_id: String) -> Result<(), String> {
    let c = require_client(&app)?;
    c.unblock(&target_id).await.map_err(|e| format!("{e:#}"))
}
