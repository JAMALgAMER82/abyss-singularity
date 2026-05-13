//! Streaming subsystem commands (Phase 5).
//!
//! We re-use the [`crate::orchestration::launcher::ProcessRegistry`] from
//! Phase 3 to spawn + track Sunshine and Moonlight as long-lived child
//! processes. That gives us identical event semantics (stdout/stderr lines
//! → `abyss://orchestration/event`) for emulators and stream binaries.

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

use super::config;
use super::types::{HostStatus, KnownHost, StreamingConfig};
use crate::orchestration::launcher::{spawn_and_track, ProcessRegistry, SpawnRequest};

/// The Sunshine host runs as a singleton — we hold its run_id here so the
/// UI can ask "is the host up?" and so we don't accidentally spawn two.
#[derive(Default)]
pub struct HostState {
    pub current_run: Mutex<Option<String>>,
}

#[derive(Serialize)]
pub struct ClientLaunchResult {
    pub run_id: String,
    pub pid: u32,
    pub command_line: String,
}

#[tauri::command]
pub fn stream_get_config<R: Runtime>(app: AppHandle<R>) -> Result<StreamingConfig, String> {
    config::load(&app).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn stream_set_config<R: Runtime>(
    app: AppHandle<R>,
    config: StreamingConfig,
) -> Result<(), String> {
    config::save(&app, &config).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn stream_add_host<R: Runtime>(
    app: AppHandle<R>,
    host: KnownHost,
) -> Result<StreamingConfig, String> {
    let mut cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    if !cfg.known_hosts.iter().any(|h| h.id == host.id) {
        cfg.known_hosts.push(host);
    }
    config::save(&app, &cfg).map_err(|e| format!("{e:#}"))?;
    Ok(cfg)
}

#[tauri::command]
pub fn stream_remove_host<R: Runtime>(
    app: AppHandle<R>,
    host_id: String,
) -> Result<StreamingConfig, String> {
    let mut cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    cfg.known_hosts.retain(|h| h.id != host_id);
    config::save(&app, &cfg).map_err(|e| format!("{e:#}"))?;
    Ok(cfg)
}

#[tauri::command]
pub fn stream_host_status<R: Runtime>(
    app: AppHandle<R>,
    host_state: State<'_, Arc<HostState>>,
    registry: State<'_, Arc<ProcessRegistry>>,
) -> Result<HostStatus, String> {
    let cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    let configured = cfg.sunshine_exe.is_some();
    let admin_url = cfg
        .sunshine_admin_url
        .clone()
        .or_else(|| Some("https://localhost:47990".into()));

    let run_id_opt = host_state.current_run.lock().expect("host_state poisoned").clone();
    let running = run_id_opt
        .as_deref()
        .map(|rid| registry.list().iter().any(|p| p.run_id == rid))
        .unwrap_or(false);
    let pid = run_id_opt
        .as_deref()
        .and_then(|rid| registry.list().into_iter().find(|p| p.run_id == rid).map(|p| p.pid));

    // If the registry no longer has it, clear our stored run_id so a
    // future "start" call doesn't think there's already a host running.
    if !running {
        *host_state.current_run.lock().expect("host_state poisoned") = None;
    }

    Ok(HostStatus {
        configured,
        running,
        pid,
        admin_url,
        run_id: if running { run_id_opt } else { None },
    })
}

#[tauri::command]
pub async fn stream_start_host<R: Runtime>(
    app: AppHandle<R>,
    host_state: State<'_, Arc<HostState>>,
    registry: State<'_, Arc<ProcessRegistry>>,
) -> Result<HostStatus, String> {
    // Don't double-spawn.
    if let Some(rid) = host_state.current_run.lock().expect("host_state poisoned").clone() {
        if registry.list().iter().any(|p| p.run_id == rid) {
            return Err("Sunshine host is already running".into());
        }
    }

    let cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    let exe = cfg
        .sunshine_exe
        .clone()
        .ok_or_else(|| "Sunshine exe path not configured — set it under Settings > Streaming".to_string())?;

    let handle = spawn_and_track(
        app.clone(),
        registry.inner().clone(),
        SpawnRequest {
            emulator_id: "sunshine-host".into(),
            entry_id:    "sunshine".into(),
            exe,
            args:        vec![],
            working_dir: None,
            env:         Default::default(),
        },
    )
    .await
    .map_err(|e| format!("{e:#}"))?;

    *host_state.current_run.lock().expect("host_state poisoned") = Some(handle.run_id.clone());

    Ok(HostStatus {
        configured: true,
        running:    true,
        pid:        Some(handle.pid),
        admin_url:  cfg.sunshine_admin_url.or_else(|| Some("https://localhost:47990".into())),
        run_id:     Some(handle.run_id),
    })
}

#[tauri::command]
pub fn stream_stop_host(
    host_state: State<'_, Arc<HostState>>,
    registry: State<'_, Arc<ProcessRegistry>>,
) -> Result<bool, String> {
    let run_id = host_state
        .current_run
        .lock()
        .expect("host_state poisoned")
        .clone();
    let killed = run_id.as_deref().map(|rid| registry.kill(rid)).unwrap_or(false);
    if killed {
        *host_state.current_run.lock().expect("host_state poisoned") = None;
    }
    Ok(killed)
}

#[tauri::command]
pub async fn stream_launch_client<R: Runtime>(
    app: AppHandle<R>,
    registry: State<'_, Arc<ProcessRegistry>>,
    host: Option<String>,
) -> Result<ClientLaunchResult, String> {
    let cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    let exe = cfg
        .moonlight_exe
        .clone()
        .ok_or_else(|| "Moonlight exe path not configured — set it under Settings > Streaming".to_string())?;

    // If a host is supplied, hand it to Moonlight as a positional arg
    // (`moonlight stream <host>` is the supported CLI shape; the bare
    // `moonlight` invocation opens the GUI host picker).
    let args = match host.as_deref() {
        Some(h) if !h.is_empty() => vec!["stream".into(), h.to_string()],
        _ => vec![],
    };

    let handle = spawn_and_track(
        app.clone(),
        registry.inner().clone(),
        SpawnRequest {
            emulator_id: "moonlight-client".into(),
            entry_id:    host.unwrap_or_else(|| "picker".to_string()),
            exe,
            args,
            working_dir: None,
            env:         Default::default(),
        },
    )
    .await
    .map_err(|e| format!("{e:#}"))?;

    Ok(ClientLaunchResult {
        run_id: handle.run_id,
        pid: handle.pid,
        command_line: handle.command_line,
    })
}
