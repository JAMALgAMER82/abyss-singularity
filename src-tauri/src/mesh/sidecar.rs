//! Sidecar spawn + supervision.
//!
//! Uses the `tauri-plugin-shell` sidecar API to start the bundled
//! `abyss-mesh` binary at app launch. The child is kept alive for the
//! lifetime of the app; if it exits unexpectedly we log and let the next
//! [`status`] call fail open (Rust UI shows "mesh offline").

use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, Result};
use tauri::{AppHandle, Runtime};
use tauri_plugin_shell::{
    process::{CommandChild, CommandEvent},
    ShellExt,
};

use super::types::MeshPorts;

#[derive(Default)]
pub struct SidecarHandle {
    pub child: Mutex<Option<CommandChild>>,
    pub ports: MeshPorts,
}

static HANDLE: OnceLock<Arc<SidecarHandle>> = OnceLock::new();
pub fn handle() -> Arc<SidecarHandle> {
    HANDLE
        .get_or_init(|| Arc::new(SidecarHandle::default()))
        .clone()
}

/// Spawn the sidecar with default ports. Call once during app setup.
pub fn spawn<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    let ports = MeshPorts::default();

    let (mut rx, child) = app
        .shell()
        .sidecar("abyss-mesh")
        .context("locating sidecar binary 'abyss-mesh' — was it bundled?")?
        .args([
            "--ctl".to_string(),   ports.control.to_string(),
            "--socks".to_string(), ports.socks5.to_string(),
            "--chat".to_string(),  ports.chat.to_string(),
        ])
        .spawn()
        .context("spawning abyss-mesh sidecar")?;

    handle().child.lock().expect("sidecar handle poisoned").replace(child);
    log::info!("mesh: sidecar spawned on ctl={} socks={} chat={}",
        ports.control, ports.socks5, ports.chat);

    // Drain the child's stdout/stderr into our log stream so any panic
    // or backtrace from the Go side is visible in `abyss-singularity.log`.
    let app_label = app.config().identifier.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(ev) = rx.recv().await {
            match ev {
                CommandEvent::Stdout(line) => {
                    log::info!("[mesh-out] {}", String::from_utf8_lossy(&line));
                }
                CommandEvent::Stderr(line) => {
                    log::warn!("[mesh-err] {}", String::from_utf8_lossy(&line));
                }
                CommandEvent::Error(err) => {
                    log::error!("[mesh-error] {err}");
                }
                CommandEvent::Terminated(payload) => {
                    log::warn!("[mesh-exit] app={app_label} code={:?} signal={:?}",
                        payload.code, payload.signal);
                    break;
                }
                _ => {}
            }
        }
    });

    Ok(())
}

/// Best-effort shutdown — sends `/shutdown` and falls back to a kill.
#[allow(dead_code)] // reserved for an explicit pre-exit hook in a later phase
pub fn shutdown() {
    let h = handle();
    // Pull the child out *before* the if-let so the MutexGuard's temporaries
    // are dropped before we touch `h.ports` in the body.
    let child_opt = h.child.lock().expect("sidecar handle poisoned").take();
    if let Some(child) = child_opt {
        let ports = h.ports;
        tauri::async_runtime::spawn(async move {
            let _ = super::control::shutdown(ports).await;
        });
        let _ = child.kill();
    }
}
