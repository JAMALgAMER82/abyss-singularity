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
    let authkey = crate::network::config::load(app)
        .ok()
        .and_then(|c| c.redeemed_authkey);
    spawn_with_authkey(app, authkey.as_deref())
}

/// Spawn the sidecar, optionally passing a Tailscale pre-auth key so
/// tsnet authenticates against an existing tailnet rather than asking
/// the user to sign in via browser. Used by both startup (which loads
/// any persisted redeemed key) and the invite-redeem flow (which calls
/// us directly with a freshly-decoded key).
pub fn spawn_with_authkey<R: Runtime>(app: &AppHandle<R>, authkey: Option<&str>) -> Result<()> {
    let ports = MeshPorts::default();

    let mut args = vec![
        "--ctl".to_string(),   ports.control.to_string(),
        "--socks".to_string(), ports.socks5.to_string(),
        "--chat".to_string(),  ports.chat.to_string(),
    ];
    if let Some(k) = authkey {
        if !k.is_empty() {
            args.push("--authkey".into());
            args.push(k.to_string());
        }
    }

    let (mut rx, child) = app
        .shell()
        .sidecar("abyss-mesh")
        .context("locating sidecar binary 'abyss-mesh' — was it bundled?")?
        .args(args)
        .spawn()
        .context("spawning abyss-mesh sidecar")?;

    handle().child.lock().expect("sidecar handle poisoned").replace(child);
    log::info!("mesh: sidecar spawned on ctl={} socks={} chat={} (authkey={})",
        ports.control, ports.socks5, ports.chat,
        if authkey.unwrap_or("").is_empty() { "no" } else { "yes" });

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

/// Kill the running sidecar (if any) and respawn it with a freshly-
/// supplied Tailscale pre-auth key. Used right after the user redeems
/// an invite so tsnet authenticates against the inviter's tailnet
/// without forcing them through a browser sign-in.
///
/// Tailscale's state lives under `%LocalAppData%\AbyssSingularity\tailscale`;
/// we wipe that subtree before respawning so the old identity doesn't
/// shadow the new pre-auth key. This is the one-line "join my friend's
/// tailnet" gesture the lobby UI builds on.
pub fn respawn_with_authkey<R: Runtime>(app: &AppHandle<R>, authkey: &str) -> Result<()> {
    // 1. Kill the current child. We need to drop the mutex guard before
    //    awaiting anything so we don't hold the lock across .kill().
    let prev = handle().child.lock().expect("sidecar handle poisoned").take();
    if let Some(child) = prev {
        let _ = child.kill();
    }
    // 2. Wipe the persisted tsnet state so the old identity doesn't
    //    win the race against the new auth key.
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let state = std::path::PathBuf::from(local)
            .join("AbyssSingularity")
            .join("tailscale");
        let _ = std::fs::remove_dir_all(&state);
        log::info!("mesh: cleared persisted tsnet state at {}", state.display());
    }
    // 3. Spawn fresh with the auth key.
    spawn_with_authkey(app, Some(authkey))
}
