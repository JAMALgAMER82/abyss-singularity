use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Persisted streaming subsystem config — paths to the user-installed
/// Sunshine and Moonlight binaries plus a list of known mesh hosts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamingConfig {
    #[serde(default)] pub sunshine_exe:  Option<PathBuf>,
    /// Sunshine's web admin URL. Defaults to https://localhost:47990 (its
    /// out-of-box bind), exposed so power users can move it.
    #[serde(default)] pub sunshine_admin_url: Option<String>,
    /// Sunshine admin username — needed to POST against /api/pin so the
    /// in-app "Pair client" button can register a PIN without making the
    /// user open the web UI. Cached after first successful pair.
    #[serde(default)] pub sunshine_admin_user: Option<String>,
    /// Matching password. Stored plaintext in settings.json (same blob
    /// where Sunshine itself stores it under config/sunshine_state.json).
    /// Local-only data; the file already lives outside the install dir.
    #[serde(default)] pub sunshine_admin_pass: Option<String>,
    #[serde(default)] pub moonlight_exe: Option<PathBuf>,
    #[serde(default)] pub known_hosts:   Vec<KnownHost>,
}

/// One paired Sunshine host the client side knows about. Acts as a quick
/// "favourites" list distinct from Moonlight's own mDNS discovery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnownHost {
    pub id:   String,
    pub name: String,
    /// IP or hostname reachable over the mesh.
    pub host: String,
}

/// What the UI shows in the "host" panel.
#[derive(Debug, Clone, Serialize)]
pub struct HostStatus {
    pub configured: bool,
    pub running:    bool,
    pub pid:        Option<u32>,
    pub admin_url:  Option<String>,
    pub run_id:     Option<String>,
}
