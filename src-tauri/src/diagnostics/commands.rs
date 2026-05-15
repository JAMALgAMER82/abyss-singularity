//! The actual diagnose-and-repair Tauri commands.

use std::time::Instant;

use tauri::{AppHandle, Runtime, Manager};

use super::types::{CheckResult, CheckStatus, DiagnosticsReport};

/// Run every self-heal Abyss knows about, in order, and report back.
/// Each step is best-effort and never aborts the rest of the run — the
/// user gets a complete picture even if one step fails.
#[tauri::command]
pub async fn diagnostics_run_all<R: Runtime>(app: AppHandle<R>) -> Result<DiagnosticsReport, String> {
    let started = Instant::now();
    let mut checks: Vec<CheckResult> = Vec::new();

    // -- 1. Mesh sidecar ---------------------------------------------------
    checks.push(check_mesh_sidecar(&app).await);

    // -- 2. Emulator install/repair ---------------------------------------
    match crate::installer::commands::installer_repair(app.clone()) {
        Ok(n) if n > 0 => checks.push(CheckResult::repaired(
            "emulator-paths",
            "Emulator install paths",
            format!("Refreshed exe locations for {n} emulator(s) after install repair."),
        )),
        Ok(_)  => checks.push(CheckResult::ok(
            "emulator-paths",
            "Emulator install paths",
            "All installed emulators already have valid exe paths.",
        )),
        Err(e) => checks.push(CheckResult::failed(
            "emulator-paths",
            "Emulator install paths",
            format!("installer_repair failed: {e}"),
        )),
    }

    // -- 3. Missing emulators get auto-installed --------------------------
    checks.push(check_missing_emulators(&app).await);

    // -- 4. RetroArch cores bundle ----------------------------------------
    checks.push(check_retroarch_cores(&app));

    // -- 5. BIOS auto-finder ---------------------------------------------
    match crate::installer::bios_finder::auto_install_all() {
        Ok(found) if !found.is_empty() => {
            let labels: Vec<&str> = found.keys().map(|s| s.as_str()).collect();
            checks.push(CheckResult::repaired(
                "bios-found",
                "Console BIOS auto-find",
                format!("Located + installed BIOS for: {}", labels.join(", ")),
            ));
        }
        Ok(_)  => checks.push(check_bios_status(&app)),
        Err(e) => checks.push(CheckResult::failed(
            "bios-found",
            "Console BIOS auto-find",
            format!("scan failed: {e}"),
        )),
    }

    // -- 6. Controller defaults ------------------------------------------
    let configured = crate::installer::controller_setup::apply_all_defaults();
    if configured.is_empty() {
        checks.push(CheckResult::ok(
            "controller-defaults",
            "Controller defaults",
            "PCSX2, DuckStation, and Dolphin gamepad bindings already in place.",
        ));
    } else {
        checks.push(CheckResult::repaired(
            "controller-defaults",
            "Controller defaults",
            format!("Seeded gamepad config for: {}", configured.join(", ")),
        ));
    }

    // -- 7. Sunshine streaming host + Moonlight client --------------------
    checks.push(check_sunshine());
    checks.push(check_moonlight());

    // -- 8. PS3 firmware status (informational; the actual install is a
    //       separate explicit command because it's a 200 MB download) ---
    checks.push(check_ps3_firmware(&app));

    let repaired_count   = checks.iter().filter(|c| matches!(c.status, CheckStatus::Repaired)).count();
    let needs_user_count = checks.iter().filter(|c| matches!(c.status, CheckStatus::NeedsUser)).count();
    let failed_count     = checks.iter().filter(|c| matches!(c.status, CheckStatus::Failed)).count();

    Ok(DiagnosticsReport {
        checks,
        elapsed_ms: started.elapsed().as_millis() as u64,
        repaired_count,
        needs_user_count,
        failed_count,
    })
}

// -------------------------- individual checks -------------------------------

async fn check_mesh_sidecar<R: Runtime>(app: &AppHandle<R>) -> CheckResult {
    // First: is the control API responding?
    let healthy = crate::mesh::control::health(crate::mesh::types::MeshPorts::default()).await;
    if healthy {
        return CheckResult::ok(
            "mesh",
            "Mesh sidecar (abyss-mesh.exe)",
            "Mesh sidecar is alive and the control API is responding.",
        );
    }

    // Down — try to spawn it.
    if let Err(e) = crate::mesh::sidecar::spawn(app) {
        // Check whether the binary even exists where we expect; missing
        // file is almost always antivirus quarantine.
        let bin_path = sidecar_path(app);
        let exists = bin_path.as_ref().map(|p| p.exists()).unwrap_or(false);
        if !exists {
            let p = bin_path.map(|p| p.display().to_string()).unwrap_or_default();
            return CheckResult::needs_user(
                "mesh",
                "Mesh sidecar (abyss-mesh.exe)",
                "The abyss-mesh.exe binary is missing from the install folder. \
                 Windows Defender or another antivirus probably quarantined it on install \
                 (unsigned Go binaries trip false positives). Add an exception for the \
                 path shown below, then reinstall Abyss.",
            ).with_path(p)
             .with_url("https://support.microsoft.com/en-us/windows/add-an-exclusion-to-windows-security-811816c0-4dfd-af4a-47e4-c301afe13b26");
        }
        return CheckResult::failed(
            "mesh",
            "Mesh sidecar (abyss-mesh.exe)",
            format!("Couldn't start the mesh sidecar: {e}"),
        );
    }

    // Spawn returned Ok — give it a few seconds to come up, then re-check.
    for _ in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if crate::mesh::control::health(crate::mesh::types::MeshPorts::default()).await {
            return CheckResult::repaired(
                "mesh",
                "Mesh sidecar (abyss-mesh.exe)",
                "Mesh sidecar wasn't running — Abyss restarted it.",
            );
        }
    }
    let p = sidecar_path(app).map(|p| p.display().to_string()).unwrap_or_default();
    CheckResult::needs_user(
        "mesh",
        "Mesh sidecar (abyss-mesh.exe)",
        "The mesh sidecar was launched but its control API never responded. \
         The most common cause is an antivirus product silently blocking it. \
         Add an exclusion for the path shown below, then run Repair again.",
    ).with_path(p)
}

fn sidecar_path<R: Runtime>(app: &AppHandle<R>) -> Option<std::path::PathBuf> {
    // tauri-plugin-shell sidecars get extracted next to the main exe at
    // install time. Resolve via the current exe path.
    let main_exe = app.path().resource_dir().ok()?;
    // tauri places the sidecar at <resource_dir>/abyss-mesh.exe on Windows.
    let candidate = main_exe.join("abyss-mesh.exe");
    Some(candidate)
}

async fn check_missing_emulators<R: Runtime>(app: &AppHandle<R>) -> CheckResult {
    let manifests = crate::installer::manifests::all();
    let total = manifests.len();
    let mut installed = 0usize;
    let mut missing: Vec<String> = Vec::new();
    let root = match app.path().app_data_dir() {
        Ok(d) => d.join("emulators"),
        Err(e) => return CheckResult::failed(
            "emulators",
            "Emulator binaries",
            format!("Couldn't resolve emulator install dir: {e}"),
        ),
    };
    for m in &manifests {
        let dir = root.join(&m.id);
        if dir.exists() {
            installed += 1;
        } else {
            missing.push(m.id.clone());
        }
    }
    if missing.is_empty() {
        return CheckResult::ok(
            "emulators",
            "Emulator binaries",
            format!("All {total} emulator(s) installed."),
        );
    }
    // Best-effort install of the missing ones. This can take many minutes
    // on a fresh install (~500 MB of downloads); the UI should warn before
    // kicking diagnostics off the first time.
    let mut new_count = 0usize;
    for id in &missing {
        if crate::installer::commands::installer_install(app.clone(), id.clone()).await.is_ok() {
            new_count += 1;
        }
    }
    if new_count == missing.len() {
        CheckResult::repaired(
            "emulators",
            "Emulator binaries",
            format!("Installed {new_count} missing emulator(s) ({installed} were already present)."),
        )
    } else {
        CheckResult::needs_user(
            "emulators",
            "Emulator binaries",
            format!(
                "{new_count} of {} missing emulator(s) installed; {} failed (network down or upstream URL changed). Re-run Repair when online.",
                missing.len(),
                missing.len() - new_count,
            ),
        )
    }
}

fn check_retroarch_cores<R: Runtime>(app: &AppHandle<R>) -> CheckResult {
    let root = match app.path().app_data_dir() {
        Ok(d) => d.join("emulators").join("retroarch").join("RetroArch-Win64").join("cores"),
        Err(e) => return CheckResult::failed(
            "retroarch-cores", "RetroArch cores",
            format!("Couldn't resolve cores dir: {e}"),
        ),
    };
    if !root.exists() {
        return CheckResult::needs_user(
            "retroarch-cores",
            "RetroArch cores",
            "RetroArch isn't installed yet — install it from Settings → Emulators or run Repair to install everything.",
        );
    }
    let count = std::fs::read_dir(&root).map(|rd|
        rd.filter_map(Result::ok)
          .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("dll"))
          .filter(|e| e.file_name().to_string_lossy().ends_with("_libretro.dll"))
          .count()
    ).unwrap_or(0);
    if count >= 50 {
        CheckResult::ok(
            "retroarch-cores",
            "RetroArch cores",
            format!("{count} libretro cores installed."),
        )
    } else {
        CheckResult::needs_user(
            "retroarch-cores",
            "RetroArch cores",
            format!(
                "Only {count} libretro cores found; the bundle didn't fully extract. Re-install RetroArch from Settings → Emulators."
            ),
        ).with_path(root.display().to_string())
    }
}

fn check_bios_status<R: Runtime>(app: &AppHandle<R>) -> CheckResult {
    let user_profile = std::env::var_os("USERPROFILE").map(std::path::PathBuf::from).unwrap_or_default();
    let local_app    = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from).unwrap_or_default();
    let mut have: Vec<&str> = Vec::new();
    let mut missing: Vec<&str> = Vec::new();

    let ps1 = local_app.join("DuckStation").join("bios");
    if ps1.exists() && std::fs::read_dir(&ps1).map(|r| r.count() > 0).unwrap_or(false) {
        have.push("PS1");
    } else { missing.push("PS1"); }

    let ps2 = user_profile.join("Documents").join("PCSX2").join("bios");
    if ps2.exists() && std::fs::read_dir(&ps2).map(|r| r.count() > 0).unwrap_or(false) {
        have.push("PS2");
    } else { missing.push("PS2"); }

    let ps3 = app.path()
        .app_data_dir().ok()
        .map(|d| d.join("emulators").join("rpcs3").join("dev_flash").join("sys"));
    if let Some(p) = ps3.as_ref() {
        if p.exists() { have.push("PS3"); } else { missing.push("PS3"); }
    }

    if missing.is_empty() {
        return CheckResult::ok("bios", "Console BIOS / firmware",
            format!("BIOS in place for: {}", have.join(", ")));
    }
    CheckResult::needs_user(
        "bios",
        "Console BIOS / firmware",
        format!(
            "Missing BIOS for: {}. Abyss can't legally ship these — see Settings for the legal-dump guides. \
             PS3 firmware can be auto-installed via Sony's free public download (one click below).",
            missing.join(", "),
        ),
    )
}

/// Check whether Moonlight is installed; if not, surface as needs-user.
/// (Auto-install for both Sunshine + Moonlight is exposed as a separate
/// explicit command — Sunshine's UAC prompt means we don't want it to
/// fire as part of a routine diagnostics pass.)
fn check_moonlight() -> CheckResult {
    let m = crate::installer::streaming_apps::detect_existing();
    if m.moonlight_installed {
        CheckResult::ok("moonlight", "Moonlight streaming client",
            "Moonlight is installed.")
    } else {
        CheckResult::needs_user(
            "moonlight",
            "Moonlight streaming client",
            "Moonlight isn't installed — click 'Install Sunshine + Moonlight' in Settings → Streaming. \
             Abyss will download both directly from their official GitHub releases (one UAC prompt for Sunshine).",
        )
    }
}

fn check_sunshine() -> CheckResult {
    #[cfg(target_os = "windows")]
    {
        let out = match crate::util::silent_cmd_std("sc").args(["query", "SunshineService"]).output() {
            Ok(o) => o,
            Err(e) => return CheckResult::failed(
                "sunshine", "Sunshine streaming host",
                format!("Couldn't query SunshineService: {e}"),
            ),
        };
        if !out.status.success() {
            return CheckResult::needs_user(
                "sunshine",
                "Sunshine streaming host",
                "Sunshine isn't installed — streaming is optional, install it from Settings → Streaming.",
            );
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.contains("RUNNING") {
            return CheckResult::ok(
                "sunshine", "Sunshine streaming host",
                "SunshineService is running. Admin UI at https://localhost:47990",
            );
        }
        // Service exists but isn't running — try to start.
        let r = crate::util::silent_cmd_std("sc").args(["start", "SunshineService"]).output();
        match r {
            Ok(o) if o.status.success() => CheckResult::repaired(
                "sunshine", "Sunshine streaming host",
                "SunshineService was stopped — Abyss started it.",
            ),
            _ => CheckResult::needs_user(
                "sunshine", "Sunshine streaming host",
                "SunshineService is installed but couldn't be started without admin rights. \
                 Right-click Abyss → Run as administrator, then click Repair again.",
            ),
        }
    }
    #[cfg(not(target_os = "windows"))]
    CheckResult::skipped("sunshine", "Sunshine streaming host", "Windows-only check.")
}

fn check_ps3_firmware<R: Runtime>(app: &AppHandle<R>) -> CheckResult {
    let dev_flash = app.path()
        .app_data_dir().ok()
        .map(|d| d.join("emulators").join("rpcs3").join("dev_flash").join("sys"));
    match dev_flash {
        Some(p) if p.exists() => CheckResult::ok(
            "ps3-firmware", "PS3 firmware (RPCS3)",
            "Sony PS3 firmware installed. RPCS3 ready.",
        ),
        Some(_) => CheckResult::needs_user(
            "ps3-firmware",
            "PS3 firmware (RPCS3)",
            "RPCS3 needs Sony's PS3 firmware to boot any PS3 game. Sony publishes it for free — \
             click 'Install PS3 firmware' in Settings → Emulators (one click, ~200 MB).",
        ),
        None => CheckResult::skipped(
            "ps3-firmware", "PS3 firmware (RPCS3)",
            "RPCS3 not installed yet.",
        ),
    }
}
