//! Tauri commands for the controller subsystem.

use std::path::PathBuf;

use tauri::{AppHandle, Runtime};

use super::retroarch;
use super::types::{AutoConfigReport, ControllerKind};
use crate::orchestration::config as orch_config;

/// Public detection helper — exposed so the frontend can ask the
/// backend's verdict instead of re-implementing the heuristic.
#[tauri::command]
pub fn controller_detect_kind(id: String) -> ControllerKind {
    ControllerKind::detect_from_id(&id)
}

/// Write a RetroArch joypad autoconfig file for the given controller.
///
/// The file lands under `<retroarch_install_dir>/autoconfig/<driver>/`
/// so RetroArch auto-loads it on next launch. If `force=false` and a
/// matching file already exists, leaves it alone (RetroArch ships a
/// large bundled autoconfig database — we don't want to clobber a
/// hand-tuned config).
#[tauri::command]
pub fn controller_apply_to_retroarch<R: Runtime>(
    app:             AppHandle<R>,
    kind:            ControllerKind,
    controller_name: String,
    force:           Option<bool>,
) -> Result<AutoConfigReport, String> {
    let force = force.unwrap_or(false);

    // 1. Find the installed RetroArch via the orchestration config.
    let cfg = orch_config::load(&app).map_err(|e| format!("{e:#}"))?;
    let entry = cfg.emulators.iter().find(|e| e.id == "retroarch").cloned()
        .ok_or_else(|| "RetroArch isn't installed — go to Settings > Emulator manager".to_string())?;
    if entry.exe.as_os_str().is_empty() {
        return Err("RetroArch is registered but its exe path is empty".into());
    }
    let retroarch_dir = entry.exe.parent().map(PathBuf::from)
        .ok_or_else(|| "RetroArch exe has no parent dir".to_string())?;

    // 2. Compose the target path.
    let driver_dir = retroarch_dir.join("autoconfig").join(kind.retroarch_driver());
    std::fs::create_dir_all(&driver_dir)
        .map_err(|e| format!("create {}: {e}", driver_dir.display()))?;
    let file = driver_dir.join(format!("{}.cfg", retroarch::safe_filename(&controller_name)));

    if file.exists() && !force {
        return Err(format!(
            "autoconfig already exists at {} (pass force=true to overwrite)",
            file.display(),
        ));
    }

    // 3. Generate + write.
    let body = retroarch::build_config(kind, &controller_name);
    let bytes = body.len();
    std::fs::write(&file, body)
        .map_err(|e| format!("write {}: {e}", file.display()))?;

    log::info!("controller: wrote autoconfig {} ({bytes} bytes) for kind={kind:?}", file.display());
    Ok(AutoConfigReport { written_to: file, bytes })
}
