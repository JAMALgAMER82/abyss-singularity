use std::collections::BTreeMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::library::types::Platform;

/// One emulator entry that the user has configured. The user picks where
/// the binary lives on disk; we own the argument template + the set of
/// platforms it can handle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmulatorEntry {
    pub id: String,
    pub name: String,
    pub exe: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub platforms: Vec<Platform>,
}

/// Top-level orchestration config persisted via `tauri-plugin-store` under
/// the key [`super::config::ORCHESTRATION_CONFIG_KEY`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrchestrationConfig {
    #[serde(default)]
    pub emulators: Vec<EmulatorEntry>,
    /// Platform → emulator-id mapping. The chosen emulator is what runs
    /// when a user clicks "Play" on a game of that platform.
    #[serde(default)]
    pub assignments: BTreeMap<Platform, String>,
}

/// Returned from `orch_launch` so the UI can correlate subsequent events.
#[derive(Debug, Clone, Serialize)]
pub struct LaunchHandle {
    pub run_id: String,
    pub pid: u32,
    pub started_at: DateTime<Utc>,
    pub emulator_id: String,
    pub entry_id: String,
    pub command_line: String,
}

/// Event emitted while a game runs. JSON-tagged so the frontend can
/// switch on `kind` cleanly.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LaunchEvent {
    Stdout { run_id: String, line: String },
    Stderr { run_id: String, line: String },
    Exited { run_id: String, code: Option<i32> },
}

/// A snapshot of one process that's currently running.
#[derive(Debug, Clone, Serialize)]
pub struct RunningProcess {
    pub run_id: String,
    pub pid: u32,
    pub started_at: DateTime<Utc>,
    pub emulator_id: String,
    pub entry_id: String,
}
