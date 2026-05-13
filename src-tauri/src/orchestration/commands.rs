//! Tauri commands for emulator orchestration.

use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime, State};

use super::config;
use super::launcher::{spawn_and_track, ProcessRegistry, SpawnRequest};
use super::recipes::{builtin_recipes, expand_args};
use super::types::{EmulatorEntry, LaunchHandle, OrchestrationConfig, RunningProcess};
use crate::library::cache;

#[tauri::command]
pub fn orch_get_config<R: Runtime>(app: AppHandle<R>) -> Result<OrchestrationConfig, String> {
    config::load(&app).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn orch_set_config<R: Runtime>(
    app: AppHandle<R>,
    config: OrchestrationConfig,
) -> Result<(), String> {
    config::save(&app, &config).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
pub fn orch_builtin_recipes() -> Vec<EmulatorEntry> {
    builtin_recipes()
}

#[tauri::command]
pub async fn orch_launch<R: Runtime>(
    app: AppHandle<R>,
    registry: State<'_, Arc<ProcessRegistry>>,
    entry_id: String,
) -> Result<LaunchHandle, String> {
    // 1. Resolve the library entry by id.
    let app_for_data = app.clone();
    let dir = app_for_data
        .path()
        .app_data_dir()
        .map_err(|e| format!("resolving app data dir: {e}"))?;
    let snapshot = cache::load(&dir).map_err(|e| format!("{e:#}"))?;
    let entry = snapshot
        .entries
        .into_iter()
        .find(|e| e.id == entry_id)
        .ok_or_else(|| format!("library entry not found: {entry_id}"))?;

    // 2. Look up the platform's assigned emulator.
    let cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    let emulator_id = cfg
        .assignments
        .get(&entry.platform)
        .cloned()
        .ok_or_else(|| {
            format!(
                "no emulator assigned to platform {:?} — set one under Settings > Emulators",
                entry.platform
            )
        })?;
    let emulator = cfg
        .emulators
        .iter()
        .find(|e| e.id == emulator_id)
        .cloned()
        .ok_or_else(|| format!("assigned emulator {emulator_id} is missing from config"))?;

    if emulator.exe.as_os_str().is_empty() {
        return Err(format!(
            "emulator {} has no exe path set — choose one under Settings",
            emulator.name
        ));
    }

    // 3. Expand args and launch.
    let args = expand_args(&emulator.args, &entry);
    // For PC platforms the emulator is "pc-direct" and the game itself is the exe.
    let exe = if emulator.id == "pc-direct" {
        entry.path.clone()
    } else {
        emulator.exe.clone()
    };

    let handle = spawn_and_track(
        app.clone(),
        registry.inner().clone(),
        SpawnRequest {
            emulator_id: emulator.id.clone(),
            entry_id:    entry.id.clone(),
            exe,
            args,
            working_dir: emulator.working_dir,
            env:         emulator.env,
        },
    )
    .await
    .map_err(|e| format!("{e:#}"))?;

    // Try to embed the emulator window into the Abyss main window when
    // the emulator is one we've confirmed reparents cleanly. The user's
    // "everything inside the app" requirement lands here.
    #[cfg(target_os = "windows")]
    {
        if super::recipes::is_embeddable(&emulator.id) {
            if let Some(host_hwnd) = main_window_hwnd(&app) {
                let pid = handle.pid;
                tauri::async_runtime::spawn(async move {
                    use std::time::Duration;
                    if let Err(e) = super::embed::embed_window(host_hwnd, pid, Duration::from_secs(8)).await {
                        log::warn!("orch: window embed failed for pid {pid}: {e:#}");
                    } else {
                        log::info!("orch: embedded pid {pid} into main window");
                    }
                });
            }
        }
    }

    Ok(handle)
}

#[cfg(target_os = "windows")]
fn main_window_hwnd<R: Runtime>(app: &AppHandle<R>) -> Option<isize> {
    use tauri::Manager as _;
    let win = app.get_webview_window("main")?;
    let raw = win.hwnd().ok()?;
    Some(raw.0 as isize)
}

#[tauri::command]
pub fn orch_terminate(
    registry: State<'_, Arc<ProcessRegistry>>,
    run_id: String,
) -> Result<bool, String> {
    Ok(registry.kill(&run_id))
}

#[tauri::command]
pub fn orch_list_running(
    registry: State<'_, Arc<ProcessRegistry>>,
) -> Result<Vec<RunningProcess>, String> {
    Ok(registry.list())
}
