//! Streaming subsystem commands (Phase 5).
//!
//! We re-use the [`crate::orchestration::launcher::ProcessRegistry`] from
//! Phase 3 to spawn + track Sunshine and Moonlight as long-lived child
//! processes. That gives us identical event semantics (stdout/stderr lines
//! → `abyss://orchestration/event`) for emulators and stream binaries.
//!
//! Sunshine note: the official Windows installer registers a Local System
//! service ("SunshineService") that owns the display-capture privileges.
//! Spawning sunshine.exe as a regular user crashes (access violation), so
//! when that service is present we drive it via `sc` instead of forking a
//! second copy.

use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};

use super::config;
use super::types::{HostStatus, KnownHost, StreamingConfig};
use crate::orchestration::launcher::{spawn_and_track, ProcessRegistry, SpawnRequest};

#[cfg(target_os = "windows")]
const SUNSHINE_SERVICE: &str = "SunshineService";
/// Apollo (https://github.com/ClassicOldSong/Apollo) is a Sunshine fork
/// that registers under its own service name. We detect either so a user
/// who's manually swapped to Apollo gets the same start/stop UX.
#[cfg(target_os = "windows")]
const APOLLO_SERVICE:  &str = "ApolloService";

/// Query whether the Sunshine OR Apollo service exists *and* is currently
/// running. Returns `(exists, running)`. Both default to false on
/// non-Windows or when `sc` is unavailable.
#[cfg(target_os = "windows")]
fn sunshine_service_state() -> (bool, bool) {
    for name in [SUNSHINE_SERVICE, APOLLO_SERVICE] {
        if let Some(state) = query_service(name) {
            return state;
        }
    }
    (false, false)
}

#[cfg(target_os = "windows")]
fn query_service(name: &str) -> Option<(bool, bool)> {
    let out = crate::util::silent_cmd_std("sc").args(["query", name]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    Some((true, stdout.contains("RUNNING")))
}

#[cfg(not(target_os = "windows"))]
fn sunshine_service_state() -> (bool, bool) { (false, false) }

#[cfg(target_os = "windows")]
fn sunshine_service_command(verb: &str) -> Result<(), String> {
    // Target whichever service is actually registered. Falls back to
    // SunshineService when neither is found so the user gets a sensible
    // error referencing the canonical name.
    let target = if query_service(APOLLO_SERVICE).is_some() {
        APOLLO_SERVICE
    } else {
        SUNSHINE_SERVICE
    };
    let out = crate::util::silent_cmd_std("sc")
        .args([verb, target])
        .output()
        .map_err(|e| format!("invoking sc.exe: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        return Err(format!(
            "sc {verb} {target} failed (run Abyss as administrator to control the streaming host service): {} {}",
            stderr.trim(),
            stdout.trim(),
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn sunshine_service_command(_verb: &str) -> Result<(), String> {
    Err("Sunshine service control is Windows-only".into())
}

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
    let (svc_exists, svc_running) = sunshine_service_state();
    let configured = cfg.sunshine_exe.is_some() || svc_exists;
    let admin_url = cfg
        .sunshine_admin_url
        .clone()
        .or_else(|| Some("https://localhost:47990".into()));

    let run_id_opt = host_state.current_run.lock().expect("host_state poisoned").clone();
    let fg_running = run_id_opt
        .as_deref()
        .map(|rid| registry.list().iter().any(|p| p.run_id == rid))
        .unwrap_or(false);
    let pid = run_id_opt
        .as_deref()
        .and_then(|rid| registry.list().into_iter().find(|p| p.run_id == rid).map(|p| p.pid));

    // If the registry no longer has it, clear our stored run_id so a
    // future "start" call doesn't think there's already a host running.
    if !fg_running {
        *host_state.current_run.lock().expect("host_state poisoned") = None;
    }

    // Sunshine is "running" if either we spawned it OR the service is up.
    let running = fg_running || svc_running;

    Ok(HostStatus {
        configured,
        running,
        pid,
        admin_url,
        run_id: if fg_running { run_id_opt } else { None },
    })
}

#[tauri::command]
pub async fn stream_start_host<R: Runtime>(
    app: AppHandle<R>,
    host_state: State<'_, Arc<HostState>>,
    registry: State<'_, Arc<ProcessRegistry>>,
) -> Result<HostStatus, String> {
    // Prefer driving the Sunshine service when it's installed — it has the
    // Local System privileges needed for display capture. Spawning the exe
    // as the current user crashes immediately (access violation).
    let (svc_exists, svc_running) = sunshine_service_state();
    if svc_exists {
        if !svc_running {
            sunshine_service_command("start")?;
        }
        let cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
        return Ok(HostStatus {
            configured: true,
            running:    true,
            pid:        None,
            admin_url:  cfg.sunshine_admin_url.or_else(|| Some("https://localhost:47990".into())),
            run_id:     None,
        });
    }

    // No service — fall back to spawning sunshine.exe directly. Works on
    // portable builds where the user dropped the binary somewhere themselves.
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
    // Service-mode (Windows): stop the service.
    let (svc_exists, svc_running) = sunshine_service_state();
    if svc_exists {
        if svc_running {
            sunshine_service_command("stop")?;
        }
        return Ok(true);
    }

    // Foreground-spawn mode: kill the child we tracked.
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
    stream_launch_client_with_registry(app, registry.inner().clone(), host).await
}

/// Internal helper callable from non-Tauri-command paths (e.g. the chat
/// handler in `streaming::pairing` after a `StreamPairResult` arrives).
/// Pulls `ProcessRegistry` out of the AppHandle so callers don't have
/// to forward a `State<'_>`.
pub async fn stream_launch_client_internal<R: Runtime>(
    app:  AppHandle<R>,
    host: Option<String>,
) -> Result<ClientLaunchResult, String> {
    let registry: tauri::State<Arc<ProcessRegistry>> = app.state();
    stream_launch_client_with_registry(app.clone(), registry.inner().clone(), host).await
}

async fn stream_launch_client_with_registry<R: Runtime>(
    app:      AppHandle<R>,
    registry: Arc<ProcessRegistry>,
    host:     Option<String>,
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
        registry,
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

    // Same "one-app feel" trick we use for emulators: minimise Abyss so
    // Moonlight owns the screen. The launcher's exit watcher restores
    // the window when Moonlight quits, regardless of which subsystem
    // spawned the child.
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.minimize();
    }

    Ok(ClientLaunchResult {
        run_id: handle.run_id,
        pid: handle.pid,
        command_line: handle.command_line,
    })
}

/// Register a 4-digit pairing PIN with the local Sunshine host via its
/// REST API at `<admin_url>/api/pin`, so the user can pair a Moonlight
/// client without ever opening Sunshine's web UI. Sunshine requires
/// HTTP Basic auth against the admin credentials; we cache them in the
/// streaming config after first successful call so subsequent pairs are
/// fully in-app.
/// Result of `stream_reset_credentials`. The plaintext password is
/// returned so the UI can show it once if the user wants to copy it
/// for record-keeping; subsequent reads come from [`StreamingConfig`].
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetCredsReport {
    pub user: String,
    pub pass: String,
}

/// Idempotent "reset Sunshine admin credentials" — runs
/// `sunshine.exe --creds <user> <pass>` with a freshly-generated password,
/// brackets it with `sc stop / sc start` so the running SunshineService
/// picks up the new creds, and persists the pair into
/// [`StreamingConfig::sunshine_admin_user`] + `sunshine_admin_pass`.
///
/// Triggers a single UAC prompt because writing to
/// `%ProgramFiles%\Sunshine\config\sunshine_state.json` requires admin.
/// Designed for the case where Sunshine was installed manually (so the
/// install-time auto-setup hook never fired) — gives the user a single
/// button to flip the in-app auto-pair flow from broken to working.
#[tauri::command]
pub async fn stream_reset_credentials<R: Runtime>(app: AppHandle<R>) -> Result<ResetCredsReport, String> {
    let mut cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    let exe = cfg
        .sunshine_exe
        .clone()
        .ok_or_else(|| {
            "Sunshine isn't installed yet — open Settings → Streaming and click \
             'Install Sunshine + Moonlight' first.".to_string()
        })?;
    if !exe.exists() {
        return Err(format!(
            "Sunshine exe path is set but the file doesn't exist: {}. \
             Reinstall Sunshine via Settings → Streaming.",
            exe.display()
        ));
    }
    let user = "abyss".to_string();
    let pass = crate::installer::streaming_apps::generate_password();
    crate::installer::streaming_apps::autoset_sunshine_creds(&exe, &user, &pass)
        .await
        .map_err(|e| format!("running sunshine --creds: {e:#}"))?;
    cfg.sunshine_admin_user = Some(user.clone());
    cfg.sunshine_admin_pass = Some(pass.clone());
    config::save(&app, &cfg).map_err(|e| format!("{e:#}"))?;
    Ok(ResetCredsReport { user, pass })
}

#[tauri::command]
pub async fn stream_pair_client<R: Runtime>(
    app: AppHandle<R>,
    pin:  String,
    name: Option<String>,
    admin_user: Option<String>,
    admin_pass: Option<String>,
) -> Result<(), String> {
    let pin = pin.trim().to_string();
    if pin.len() != 4 || !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err("PIN must be exactly 4 digits.".into());
    }

    let mut cfg = config::load(&app).map_err(|e| format!("{e:#}"))?;
    let user = admin_user.or_else(|| cfg.sunshine_admin_user.clone())
        .ok_or_else(|| "Sunshine admin username not set. Fill in the credentials field once and try again.".to_string())?;
    let pass = admin_pass.or_else(|| cfg.sunshine_admin_pass.clone())
        .ok_or_else(|| "Sunshine admin password not set.".to_string())?;
    let base = cfg.sunshine_admin_url.clone().unwrap_or_else(|| "https://localhost:47990".to_string());
    let name = name.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "AbyssPaired".to_string());

    // Sunshine ships a self-signed cert; we must skip verification.
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("building http client: {e}"))?;

    let body = serde_json::json!({ "pin": pin, "name": name });
    let resp = client.post(format!("{base}/api/pin"))
        .basic_auth(&user, Some(&pass))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("calling Sunshine /api/pin: {e}"))?;
    let status = resp.status();
    let txt = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        if status.as_u16() == 401 {
            return Err("Sunshine rejected the admin credentials (401). Double-check the username/password you set in the Sunshine admin UI.".into());
        }
        return Err(format!("Sunshine returned {status}: {}", txt.trim()));
    }
    // Sunshine returns `{"status":"true"}` on success and `{"status":"false","error":"..."}` on PIN mismatch.
    if let Ok(j) = serde_json::from_str::<serde_json::Value>(&txt) {
        if j.get("status").and_then(|v| v.as_str()) == Some("false") {
            let err_msg = j.get("error").and_then(|v| v.as_str()).unwrap_or("PIN rejected.");
            return Err(format!("Sunshine: {err_msg}"));
        }
    }
    // Cache creds for next time.
    cfg.sunshine_admin_user = Some(user);
    cfg.sunshine_admin_pass = Some(pass);
    config::save(&app, &cfg).map_err(|e| format!("{e:#}"))?;
    Ok(())
}
