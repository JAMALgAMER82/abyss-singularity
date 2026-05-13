use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::library::types::Platform;

/// Archive format the download will arrive in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveFormat { Zip, SevenZ }

/// Static description of an emulator we know how to install. Bundled
/// inside the app as a hardcoded list (see [`super::manifests::all`]).
/// New entries are pure data — no code change required.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulatorManifest {
    /// Matches the `id` field in an `EmulatorEntry` so installation can
    /// auto-populate the OrchestrationConfig recipe.
    pub id:              String,
    pub name:            String,
    pub homepage:        String,
    pub license:         String,
    pub url:             String,
    pub archive_format:  ArchiveFormat,
    /// Relative path to the launchable .exe inside the extracted dir.
    /// May contain a subfolder if the archive nests its contents.
    pub exe_relpath:     String,
    pub platforms:       Vec<Platform>,
    pub approx_size_mb:  u32,
    /// Whether the emulator's window can be cleanly reparented into
    /// Abyss via Win32 SetParent. RetroArch / mGBA: yes. Modern Qt-
    /// based emulators (PCSX2 v2, RPCS3): often fails — those fall
    /// back to the "Abyss minimises while emulator runs" behaviour.
    #[serde(default)]
    pub embeddable:      bool,
}

/// What `installer_status` returns for each known manifest.
#[derive(Debug, Clone, Serialize)]
pub struct EmulatorInstallState {
    pub manifest:  EmulatorManifest,
    pub installed: bool,
    /// Path to the installed exe if `installed`.
    pub exe:       Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallReport {
    pub id:        String,
    pub exe:       PathBuf,
    pub elapsed_ms: u64,
}

/// Progress events emitted while a download / extract is running.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum InstallProgress {
    Start    { id: String },
    Download { id: String, bytes_done: u64, bytes_total: Option<u64> },
    Extract  { id: String },
    Finalize { id: String, exe: PathBuf },
    Error    { id: String, message: String },
}

pub const INSTALL_PROGRESS_EVENT: &str = "abyss://installer/progress";
