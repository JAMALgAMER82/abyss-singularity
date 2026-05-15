//! Handlers for inbound `LobbyXxx` frames over the chat channel.
//!
//! These run inside `chat::session::handle_frame` — keep them
//! non-blocking. Anything that talks to the OS / launches a process
//! moves onto the tauri async runtime.

use tauri::{AppHandle, Emitter, Runtime};

use crate::chat::state as chat_state;
use crate::chat::types::ChatProtocol;
use crate::library::types::Platform;

use super::state;
use super::types::{LOBBY_EVENT, LOBBY_INCOMING_EVENT};

/// Parse a platform value the wire sent as a snake_case string.
fn parse_platform(s: &str) -> Option<Platform> {
    serde_json::from_value::<Platform>(serde_json::Value::String(s.to_string())).ok()
}

fn emit_state<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit(LOBBY_EVENT, state::global().snapshot());
}

/// Host received "I'd like to join your room" from `peer`.
pub fn on_join_request<R: Runtime>(
    app:          &AppHandle<R>,
    peer:         &str,
    display_name: String,
) {
    let lobby = state::global();
    let Some((platform, game_name, _)) = lobby.current_room() else {
        // Not hosting — drop silently.
        return;
    };
    if !lobby.is_host() { return; }

    let changed = lobby.add_member(peer, Some(display_name));
    if changed {
        // Tell the joiner they're in.
        let chat = chat_state::global();
        if let Some(tx) = chat.peer_sender(peer) {
            let _ = tx.send(ChatProtocol::LobbyJoinAccepted {
                platform:   platform_to_wire(platform),
                game_name:  game_name.clone(),
                sent_at_ms: now_ms(),
            });
        }
        // Re-broadcast the new member list so every peer sees it.
        broadcast_advertise(app);
        emit_state(app);
    }
}

/// Member received "you're in" from the host.
pub fn on_join_accepted<R: Runtime>(
    app:        &AppHandle<R>,
    host_peer:  &str,
    platform:   String,
    game_name:  String,
) {
    let Some(p) = parse_platform(&platform) else { return; };
    let lobby = state::global();
    let host_name = chat_state::global()
        .list_peers()
        .into_iter()
        .find(|s| s.addr == host_peer)
        .and_then(|s| s.display_name);
    lobby.become_member(host_peer.to_string(), host_name, p, game_name);
    emit_state(app);
}

/// Host or member: room shutdown / leave / kick.
pub fn on_close_or_leave<R: Runtime>(app: &AppHandle<R>, from: &str) {
    let lobby = state::global();
    if lobby.is_host() {
        // A member is leaving; just drop them.
        if lobby.remove_member(from) {
            broadcast_advertise(app);
            emit_state(app);
        }
    } else {
        // We're a member and the host (or we ourselves) shut things down.
        let snapshot = lobby.snapshot();
        if snapshot.host_addr.as_deref() == Some(from) {
            lobby.clear();
            emit_state(app);
        }
    }
}

/// Host advertised they're running a room. Surface as an incoming-invite
/// event the UI can show as "Bob is hosting Mario Kart Wii — Join?".
pub fn on_advertise<R: Runtime>(
    app:       &AppHandle<R>,
    from_addr: &str,
    host_name: String,
    platform:  String,
    game_name: String,
    members:   Vec<String>,
) {
    let Some(p) = parse_platform(&platform) else { return };
    let payload = serde_json::json!({
        "host_addr":  from_addr,
        "host_name":  host_name,
        "platform":   p,
        "game_name":  game_name,
        "members":    members,
    });
    let _ = app.emit(LOBBY_INCOMING_EVENT, payload);
}

/// Host has hit Start — launch the local copy of the same game in
/// netplay-client mode pointing at `host_addr`.
pub fn on_start_game<R: Runtime>(
    app:       &AppHandle<R>,
    host_addr: String,
    platform:  String,
    game_name: String,
) {
    let Some(p) = parse_platform(&platform) else {
        log::warn!("lobby: start-game with unknown platform '{platform}'");
        return;
    };
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        match super::commands::launch_for_room(&app_clone, p, &game_name, Some(&host_addr)).await {
            Ok(_)  => log::info!("lobby: joined {game_name} hosted by {host_addr}"),
            Err(e) => log::warn!("lobby: failed to join {game_name}: {e}"),
        }
    });
}

/// Send the current room state out to every connected chat peer so the
/// room banner stays consistent. Host-only path; members just sit on the
/// last advertise they saw.
pub fn broadcast_advertise<R: Runtime>(_app: &AppHandle<R>) {
    let lobby = state::global();
    let Some((platform, game_name, members)) = lobby.current_room() else { return };
    if !lobby.is_host() { return }
    let host_name = lobby.snapshot().host_name.unwrap_or_else(|| "Abyss host".to_string());
    let member_addrs: Vec<String> = members.iter().map(|m| m.addr.clone()).collect();
    let chat = chat_state::global();
    let frame = ChatProtocol::LobbyAdvertise {
        host_name,
        platform: platform_to_wire(platform),
        game_name,
        members: member_addrs,
        sent_at_ms: now_ms(),
    };
    for peer in chat.list_peers() {
        if let Some(tx) = chat.peer_sender(&peer.addr) {
            let _ = tx.send(frame.clone());
        }
    }
}

/// Tell every member the room is over.
pub fn broadcast_close() {
    let chat = chat_state::global();
    let frame = ChatProtocol::LobbyClose { sent_at_ms: now_ms() };
    for peer in chat.list_peers() {
        if let Some(tx) = chat.peer_sender(&peer.addr) {
            let _ = tx.send(frame.clone());
        }
    }
}

/// Serialise a platform back to the snake_case the wire uses. Done via
/// serde so any future variant additions stay in sync automatically.
pub fn platform_to_wire(p: Platform) -> String {
    serde_json::to_value(p)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
