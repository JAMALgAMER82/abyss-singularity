//! Child-process orchestration.
//!
//! Spawns emulator binaries with piped stdout/stderr, streams every line
//! to the frontend as a Tauri event, and tracks live processes in an
//! in-process registry so the UI can list / kill them.
//!
//! All processes run as siblings of the Tauri main thread — the UI is
//! never blocked by emulator I/O ("master/slave" arrangement the project
//! brief calls out).

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Mutex;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;

use super::types::{LaunchEvent, LaunchHandle, RunningProcess};

/// Tauri event channel that all launcher output flows through.
pub const LAUNCH_EVENT: &str = "abyss://orchestration/event";

/// Managed Tauri state — one instance per app, holds the live process map.
#[derive(Default)]
pub struct ProcessRegistry {
    inner: Mutex<HashMap<String, RegistrySlot>>,
}

#[derive(Debug)]
struct RegistrySlot {
    pid: u32,
    emulator_id: String,
    entry_id: String,
    started_at: chrono::DateTime<chrono::Utc>,
    kill: Option<oneshot::Sender<()>>,
}

impl ProcessRegistry {
    pub fn list(&self) -> Vec<RunningProcess> {
        let g = self.inner.lock().expect("registry poisoned");
        g.iter()
            .map(|(k, v)| RunningProcess {
                run_id: k.clone(),
                pid: v.pid,
                started_at: v.started_at,
                emulator_id: v.emulator_id.clone(),
                entry_id: v.entry_id.clone(),
            })
            .collect()
    }

    pub fn kill(&self, run_id: &str) -> bool {
        let mut g = self.inner.lock().expect("registry poisoned");
        if let Some(slot) = g.get_mut(run_id) {
            if let Some(tx) = slot.kill.take() {
                let _ = tx.send(());
                return true;
            }
        }
        false
    }

    fn remove(&self, run_id: &str) {
        let mut g = self.inner.lock().expect("registry poisoned");
        g.remove(run_id);
    }
}

/// Everything `spawn_and_track` needs. Bundled so we don't blow past
/// clippy's argument-count threshold and so future fields (e.g. logging
/// scope, priority) slot in without changing the signature.
pub struct SpawnRequest {
    pub emulator_id: String,
    pub entry_id:    String,
    pub exe:         std::path::PathBuf,
    pub args:        Vec<String>,
    pub working_dir: Option<std::path::PathBuf>,
    pub env:         std::collections::BTreeMap<String, String>,
}

/// Spawn an emulator with the given expanded args, register it, and start
/// streaming I/O events. Returns immediately after spawn (does NOT wait
/// for exit).
pub async fn spawn_and_track<R: Runtime>(
    app: AppHandle<R>,
    registry: std::sync::Arc<ProcessRegistry>,
    req: SpawnRequest,
) -> Result<LaunchHandle> {
    let SpawnRequest { emulator_id, entry_id, exe, args, working_dir, env } = req;

    let run_id = format!("run-{}", uuid_like_id());
    let command_line = format!(
        "{} {}",
        exe.display(),
        args.iter()
            .map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a.clone() })
            .collect::<Vec<_>>()
            .join(" ")
    );

    log::info!("orch: launching {command_line}");

    let mut cmd = Command::new(&exe);
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(false);
    if let Some(dir) = &working_dir {
        cmd.current_dir(dir);
    }
    for (k, v) in &env {
        cmd.env(k, v);
    }

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawning emulator {}", exe.display()))?;
    let pid = child
        .id()
        .ok_or_else(|| anyhow!("could not read child pid (process exited too fast?)"))?;

    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");

    let (kill_tx, kill_rx) = oneshot::channel::<()>();

    let started_at = Utc::now();
    registry.inner.lock().expect("registry poisoned").insert(
        run_id.clone(),
        RegistrySlot {
            pid,
            emulator_id: emulator_id.clone(),
            entry_id: entry_id.clone(),
            started_at,
            kill: Some(kill_tx),
        },
    );

    // Line pumps for stdout and stderr.
    spawn_line_pump(app.clone(), run_id.clone(), stdout, true);
    spawn_line_pump(app.clone(), run_id.clone(), stderr, false);

    // Joiner: waits for either kill signal or natural exit, emits Exited,
    // removes from registry.
    let app_for_join = app.clone();
    let registry_for_join = registry.clone();
    let run_id_for_join = run_id.clone();
    tokio::spawn(async move {
        let code = tokio::select! {
            _ = kill_rx => {
                if let Err(e) = child.kill().await {
                    log::warn!("orch: kill {} failed: {e}", run_id_for_join);
                }
                child.wait().await.ok().and_then(|s| s.code())
            }
            res = child.wait() => {
                res.ok().and_then(|s| s.code())
            }
        };
        log::info!("orch: exit {} code {:?}", run_id_for_join, code);
        let _ = app_for_join.emit(
            LAUNCH_EVENT,
            LaunchEvent::Exited { run_id: run_id_for_join.clone(), code },
        );
        registry_for_join.remove(&run_id_for_join);
    });

    Ok(LaunchHandle {
        run_id,
        pid,
        started_at,
        emulator_id,
        entry_id,
        command_line,
    })
}

fn spawn_line_pump<R: Runtime, S>(
    app: AppHandle<R>,
    run_id: String,
    stream: S,
    is_stdout: bool,
) where
    S: tokio::io::AsyncRead + Send + Unpin + 'static,
{
    tokio::spawn(async move {
        let mut reader = BufReader::new(stream).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let evt = if is_stdout {
                LaunchEvent::Stdout { run_id: run_id.clone(), line }
            } else {
                LaunchEvent::Stderr { run_id: run_id.clone(), line }
            };
            let _ = app.emit(LAUNCH_EVENT, evt);
        }
    });
}

/// Tiny dependency-free unique id generator — good enough for distinguishing
/// concurrent runs in a single Tauri session.
fn uuid_like_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let counter = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{nanos:x}-{counter:x}")
}
static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
