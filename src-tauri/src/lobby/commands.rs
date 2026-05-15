//! Tauri commands exposed to the React frontend for the lobby.

use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime, State};

use crate::chat::state as chat_state;
use crate::chat::types::ChatProtocol;
use crate::library::cache;
use crate::library::scanner;
use crate::library::types::Platform;
use crate::orchestration::config as orch_config;
use crate::orchestration::launcher::{spawn_and_track, ProcessRegistry, SpawnRequest};
use crate::orchestration::recipes::{expand_args, retroarch_core_for};

use super::handlers;
use super::state;
use super::types::{LobbyLaunchReport, RoomRole, RoomSnapshot};

/// Read the current room snapshot.
#[tauri::command]
pub fn lobby_state() -> Result<RoomSnapshot, String> {
    Ok(state::global().snapshot())
}

/// Start hosting a room with the chosen game. Broadcasts a
/// `LobbyAdvertise` to every connected chat peer so they can request to
/// join. Idempotent — calling it twice with the same args refreshes the
/// advertise but doesn't reset the member list; calling with different
/// args closes the previous room first.
#[tauri::command]
pub fn lobby_host_room<R: Runtime>(
    app:       AppHandle<R>,
    platform:  Platform,
    game_name: String,
) -> Result<RoomSnapshot, String> {
    let lobby = state::global();
    let snap = lobby.snapshot();
    // If we're already hosting a different game, close the old room.
    if let Some((prev_platform, prev_name, _)) = lobby.current_room() {
        if lobby.is_host() && (prev_platform != platform || prev_name != game_name) {
            handlers::broadcast_close();
            lobby.clear();
        }
    }
    // If we were a member of someone else's room, leave it first.
    if snap.role == Some(RoomRole::Member) {
        let host = snap.host_addr.unwrap_or_default();
        if !host.is_empty() {
            send_to(&host, ChatProtocol::LobbyLeave { sent_at_ms: handlers::now_ms() });
        }
        lobby.clear();
    }
    let host_name = chat_state::global().self_name();
    lobby.become_host(platform, game_name, host_name);
    handlers::broadcast_advertise(&app);
    Ok(lobby.snapshot())
}

/// Stop hosting and tell every member the room is over.
#[tauri::command]
pub fn lobby_close_room() -> Result<RoomSnapshot, String> {
    let lobby = state::global();
    if lobby.is_host() {
        handlers::broadcast_close();
    }
    lobby.clear();
    Ok(lobby.snapshot())
}

/// Ask `host_addr` if we can join their room. Host replies asynchronously
/// via `LobbyJoinAccepted` — that's when our snapshot will flip to
/// member mode. Until then we stay in the previous role (typically None).
#[tauri::command]
pub fn lobby_request_join(host_addr: String) -> Result<(), String> {
    let chat = chat_state::global();
    let tx = chat
        .peer_sender(&host_addr)
        .ok_or_else(|| format!("not connected to {host_addr} via chat yet — click 'link' in Friends first"))?;
    let display_name = chat.self_name();
    tx.send(ChatProtocol::LobbyJoinRequest {
        display_name,
        sent_at_ms: handlers::now_ms(),
    })
    .map_err(|e| format!("sending join request: {e}"))?;
    Ok(())
}

/// Leave the room we're currently in. If we're the host this closes it
/// for everyone; if we're a member it just removes us from the host's
/// member list.
#[tauri::command]
pub fn lobby_leave_room() -> Result<RoomSnapshot, String> {
    let lobby = state::global();
    let snap = lobby.snapshot();
    if lobby.is_host() {
        handlers::broadcast_close();
    } else if let Some(host) = &snap.host_addr {
        send_to(host, ChatProtocol::LobbyLeave { sent_at_ms: handlers::now_ms() });
    }
    lobby.clear();
    Ok(lobby.snapshot())
}

/// Host hit Start — fan out `LobbyStartGame` to every member, then
/// launch our own copy as the netplay host. Members' Abysses receive
/// the start frame and auto-launch their copies as netplay clients
/// pointing at our tailnet IP.
#[tauri::command]
pub async fn lobby_start_game<R: Runtime>(
    app:      AppHandle<R>,
    registry: State<'_, Arc<ProcessRegistry>>,
    host_ip:  String,
) -> Result<LobbyLaunchReport, String> {
    let lobby = state::global();
    if !lobby.is_host() {
        return Err("only the host can start the game".into());
    }
    let (platform, game_name, members) = lobby
        .current_room()
        .ok_or_else(|| "no active room".to_string())?;

    if host_ip.trim().is_empty() {
        return Err("host IP is empty — pick your tailnet address in the Lobby panel".into());
    }

    // Fan out the start frame to every member BEFORE we minimise + launch,
    // so any "you don't have the game" failure on a member's side surfaces
    // while the host is still focused. (Members handle the frame in
    // handlers::on_start_game; failure paths log but don't bubble up to us.)
    let chat = chat_state::global();
    let frame = ChatProtocol::LobbyStartGame {
        platform:   handlers::platform_to_wire(platform),
        game_name:  game_name.clone(),
        host_addr:  host_ip.clone(),
        sent_at_ms: handlers::now_ms(),
    };
    for m in &members {
        if let Some(tx) = chat.peer_sender(&m.addr) {
            let _ = tx.send(frame.clone());
        } else {
            log::warn!("lobby: member {} has no live chat session — skipping start fan-out", m.addr);
        }
    }

    // Now launch our own copy as netplay host.
    do_launch(&app, registry.inner().clone(), platform, &game_name, None, RoomRole::Host).await
}

// ---------------------------------------------------------------------------
// Internal: shared launcher used by both Host's lobby_start_game and the
// inbound start-game handler. Keeping this in one place means the args we
// pass to RetroArch stay in sync between host and client.
// ---------------------------------------------------------------------------

pub(crate) async fn launch_for_room<R: Runtime>(
    app:       &AppHandle<R>,
    platform:  Platform,
    game_name: &str,
    join_addr: Option<&str>,
) -> Result<LobbyLaunchReport, String> {
    // Members call this via the chat handler, where we don't hold the
    // Tauri State<'_>. Pull the registry out of `app.state()` instead.
    let registry: tauri::State<Arc<ProcessRegistry>> = app.state();
    let registry = registry.inner().clone();
    let role = match join_addr {
        Some(_) => RoomRole::Member,
        None    => RoomRole::Host,
    };
    do_launch(app, registry, platform, game_name, join_addr, role).await
}

async fn do_launch<R: Runtime>(
    app:       &AppHandle<R>,
    registry:  Arc<ProcessRegistry>,
    platform:  Platform,
    game_name: &str,
    join_addr: Option<&str>,
    role:      RoomRole,
) -> Result<LobbyLaunchReport, String> {
    // 1. Resolve a library entry for this game. Library entry IDs are not
    //    stable across machines, so we look up by (platform, fuzzy name).
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("resolving app data dir: {e}"))?;
    let snapshot = cache::load(&dir).map_err(|e| format!("{e:#}"))?;
    let needle = scanner::normalise_for_hash(game_name);
    let entry = snapshot.entries.into_iter()
        .filter(|e| e.platform == platform)
        .find(|e| scanner::normalise_for_hash(&e.stem) == needle)
        .ok_or_else(|| format!(
            "you don't have a local copy of '{game_name}' for {:?}. \
             Ask the host to send it via the Friends tab, then try again.",
            platform
        ))?;

    // 2. Look up the platform's assigned emulator.
    let cfg = orch_config::load(app).map_err(|e| format!("{e:#}"))?;
    let emulator_id = cfg.assignments.get(&platform).cloned().ok_or_else(|| {
        format!("no emulator assigned to {:?} — open Settings → Emulators to pick one", platform)
    })?;
    let emulator = cfg.emulators.iter().find(|e| e.id == emulator_id).cloned()
        .ok_or_else(|| format!("emulator {emulator_id} missing from config"))?;
    if emulator.exe.as_os_str().is_empty() {
        return Err(format!("emulator {} has no exe path set", emulator.name));
    }

    // 3. Build args. Phase 12 only wires netplay for RetroArch — every
    //    libretro core gets `--host` / `--connect=` plumbing for free.
    if emulator.id != "retroarch" {
        return Err(format!(
            "Netplay in Abyss is currently RetroArch-only. The host picked '{}' \
             which is assigned to {}. Switch its emulator to RetroArch under \
             Settings → Emulators to play this together.",
            game_name, emulator.name,
        ));
    }
    let mut args = expand_args(&emulator.args, &entry);

    // Prepend the libretro core (same logic orch_launch uses).
    if let Some(core_name) = retroarch_core_for(entry.platform) {
        if let Some(core_path) = emulator.exe.parent().map(|p| p.join("cores").join(core_name)) {
            if core_path.exists() {
                args.insert(0, core_path.to_string_lossy().into_owned());
                args.insert(0, "-L".into());
            } else {
                log::warn!("retroarch core missing for {:?}: {}", entry.platform, core_path.display());
            }
        }
    }

    // Append the netplay role flag.
    match join_addr {
        Some(addr) => {
            // Client mode — connect to the host. `--connect=<host>` is the
            // long-form CLI; the short form `-C <host>` also works on
            // every RetroArch we ship (1.20+).
            args.push(format!("--connect={}", addr));
        }
        None => {
            // Host mode — start the netplay listener and wait for clients.
            args.push("--host".into());
        }
    }

    let handle = spawn_and_track(
        app.clone(),
        registry,
        SpawnRequest {
            emulator_id: emulator.id.clone(),
            entry_id:    entry.id.clone(),
            exe:         emulator.exe.clone(),
            args,
            working_dir: emulator.working_dir,
            env:         emulator.env,
        },
    )
    .await
    .map_err(|e| format!("{e:#}"))?;

    // Same "one-app feel" trick orchestration::commands uses: minimise
    // Abyss so the emulator owns the screen.
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.minimize();
    }

    Ok(LobbyLaunchReport {
        run_id:       handle.run_id,
        command_line: handle.command_line,
        role,
    })
}

/// Send a single chat frame to one peer, dropping the result silently
/// when no sender is registered (peer offline).
fn send_to(addr: &str, frame: ChatProtocol) {
    if let Some(tx) = chat_state::global().peer_sender(addr) {
        let _ = tx.send(frame);
    }
}
