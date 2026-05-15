//! Tauri commands for the Phase 6.x chat + presence subsystem.

use chrono::Utc;
use tauri::{AppHandle, Emitter, Runtime};

use super::client::connect_and_run;
use super::config;
use super::protocol::now_ms;
use super::server;
use super::session::new_message_id;
use super::state::global;
use super::types::{
    ChatConfig, ChatHistoryEntry, ChatProtocol, Direction, PeerSnapshot, PresenceStatus,
    CHAT_MESSAGE_EVENT, CHAT_PEER_EVENT,
};

#[tauri::command]
pub fn chat_get_config<R: Runtime>(app: AppHandle<R>) -> Result<ChatConfig, String> {
    config::load(&app).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn chat_set_config<R: Runtime>(app: AppHandle<R>, config: ChatConfig) -> Result<(), String> {
    if let Some(n) = config.display_name.as_deref() {
        global().set_self_name(Some(n.to_string()));
    }
    config::save(&app, &config).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub async fn chat_start<R: Runtime>(app: AppHandle<R>) -> Result<u16, String> {
    let state = global();
    let cfg   = config::load(&app).map_err(|e| format!("{e:#}"))?;
    if let Some(n) = cfg.display_name.clone() {
        state.set_self_name(Some(n));
    }
    if state.server_running() {
        return Ok(cfg.listen_port);
    }
    let app_c   = app.clone();
    let state_c = state.clone();
    let port    = cfg.listen_port;
    let handle  = tokio::spawn(async move {
        if let Err(e) = server::run(app_c, state_c, port).await {
            log::error!("chat: server task ended: {e:#}");
        }
    });
    state.install_server(handle);

    // Auto-connect: enumerate visible mesh peers and try to open a chat
    // link to each. Cheap, idempotent (`connect_and_run` is a no-op if
    // we're already linked), and silent on failure — peers that aren't
    // running Abyss just won't accept the SOCKS5 dial.
    let app_for_auto = app.clone();
    tokio::spawn(async move {
        // Brief delay so the listener has time to bind before peers
        // try the reverse direction.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        auto_connect_visible_peers(app_for_auto, port).await;
    });

    Ok(port)
}

async fn auto_connect_visible_peers<R: Runtime>(app: AppHandle<R>, port: u16) {
    let status = crate::network::tailscale::status().await;
    for peer in status.peers {
        if !peer.online { continue }
        let Some(addr) = peer.addrs.first().cloned() else { continue };
        let app_c = app.clone();
        tokio::spawn(async move {
            // Best-effort — if the peer isn't running Abyss the SOCKS5
            // dial just fails after the timeout.
            let _ = crate::chat::client::connect_and_run(
                app_c, crate::chat::state::global(), addr, port,
            ).await;
        });
    }
}

#[tauri::command]
pub fn chat_stop() -> Result<(), String> {
    global().stop_server();
    Ok(())
}

#[tauri::command]
pub fn chat_status() -> Result<ChatStatus, String> {
    let state = global();
    Ok(ChatStatus {
        running: state.server_running(),
        self_name: state.self_name(),
        presence: state.presence().0,
        activity: state.presence().1,
    })
}

#[derive(serde::Serialize)]
pub struct ChatStatus {
    pub running:   bool,
    pub self_name: String,
    pub presence:  PresenceStatus,
    pub activity:  Option<String>,
}

#[tauri::command]
pub async fn chat_connect_peer<R: Runtime>(
    app: AppHandle<R>,
    host: String,
    port: Option<u16>,
) -> Result<(), String> {
    let state = global();
    let port  = port.unwrap_or(47992);

    // Pre-flight: if the mesh sidecar already knows this peer is offline,
    // skip the 8s SOCKS5 dial that would just time out — surface a clean
    // "peer offline" message instead. We still try the dial when we can't
    // find the peer in the status (transient lookup race, fresh peer).
    let status = crate::network::tailscale::status().await;
    let peer = status.peers.iter().find(|p|
        p.addrs.iter().any(|a| a == &host)
            || p.dns_name.as_deref() == Some(host.as_str())
            || p.host_name == host
    );
    if let Some(p) = peer {
        if !p.online {
            let label = if !p.host_name.is_empty() { p.host_name.as_str() } else { host.as_str() };
            return Err(format!("{label} is offline — they need to open Abyss to receive chats."));
        }
    }

    connect_and_run(app, state, host, port)
        .await
        .map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn chat_send<R: Runtime>(
    app: AppHandle<R>,
    peer_addr: String,
    body: String,
) -> Result<ChatHistoryEntry, String> {
    let state = global();
    let tx = state
        .peer_sender(&peer_addr)
        .ok_or_else(|| format!("no live connection to {peer_addr}"))?;
    let id = new_message_id();
    let frame = ChatProtocol::Chat {
        id:         id.clone(),
        body:       body.clone(),
        sent_at_ms: now_ms(),
    };
    tx.send(frame)
        .map_err(|_| "peer connection dropped".to_string())?;
    let entry = ChatHistoryEntry {
        id,
        peer_addr,
        direction: Direction::Outbound,
        body,
        at: Utc::now(),
    };
    let stored = state.append_history(entry);
    let _ = app.emit(CHAT_MESSAGE_EVENT, stored.clone());
    Ok(stored)
}

#[tauri::command]
pub fn chat_get_history(peer_addr: Option<String>) -> Result<Vec<ChatHistoryEntry>, String> {
    Ok(global().history_for(peer_addr.as_deref()))
}

#[tauri::command]
pub fn chat_get_peers() -> Result<Vec<PeerSnapshot>, String> {
    Ok(global().list_peers())
}

#[tauri::command]
pub fn chat_set_presence<R: Runtime>(
    app: AppHandle<R>,
    status: PresenceStatus,
    activity: Option<String>,
) -> Result<(), String> {
    let state = global();
    state.set_presence(status, activity.clone());
    // Broadcast to every connected peer.
    let peers = state.list_peers();
    for p in peers {
        if !p.connected { continue }
        if let Some(tx) = state.peer_sender(&p.addr) {
            let _ = tx.send(ChatProtocol::Presence {
                status,
                activity: activity.clone(),
                sent_at_ms: now_ms(),
            });
        }
    }
    // Echo a peer-update so the local UI re-renders our own status badge.
    let _ = app.emit(CHAT_PEER_EVENT, state.list_peers());
    Ok(())
}

/// Helper for other Rust modules (e.g. orchestration) to mirror their
/// "currently launching X" event into chat presence without going through
/// the public command surface.
#[allow(dead_code)]
pub fn announce_local_activity(activity: Option<String>) {
    let state = global();
    let status = if activity.is_some() { PresenceStatus::Playing } else { PresenceStatus::Idle };
    state.set_presence(status, activity.clone());
    let peers = state.list_peers();
    for p in peers {
        if !p.connected { continue }
        if let Some(tx) = state.peer_sender(&p.addr) {
            let _ = tx.send(ChatProtocol::Presence {
                status,
                activity: activity.clone(),
                sent_at_ms: now_ms(),
            });
        }
    }
}

