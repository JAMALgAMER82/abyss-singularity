use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Stable identifier for a known gaming platform / system. New platforms
/// can be added without touching anything outside [`super::platforms`] and
/// frontend display logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Pc,
    Nes,
    Snes,
    N64,
    GameCube,
    Wii,
    WiiU,
    Switch,
    Gameboy,
    GameboyColor,
    GameboyAdvance,
    Nds,
    Threeds,
    Ps1,
    Ps2,
    Ps3,
    Psp,
    PsVita,
    Genesis,      // Sega Mega Drive
    MasterSystem,
    GameGear,
    Saturn,
    Dreamcast,
    Atari2600,
    NeoGeo,
    Arcade,       // MAME / FBNeo
    Other,
}

impl Platform {
    #[allow(dead_code)] // exercised by tests + reserved for Rust-side log formatting.
    pub fn display_name(&self) -> &'static str {
        match self {
            Platform::Pc => "PC",
            Platform::Nes => "NES",
            Platform::Snes => "SNES",
            Platform::N64 => "Nintendo 64",
            Platform::GameCube => "GameCube",
            Platform::Wii => "Wii",
            Platform::WiiU => "Wii U",
            Platform::Switch => "Switch",
            Platform::Gameboy => "Game Boy",
            Platform::GameboyColor => "Game Boy Color",
            Platform::GameboyAdvance => "Game Boy Advance",
            Platform::Nds => "Nintendo DS",
            Platform::Threeds => "Nintendo 3DS",
            Platform::Ps1 => "PlayStation",
            Platform::Ps2 => "PlayStation 2",
            Platform::Ps3 => "PlayStation 3",
            Platform::Psp => "PSP",
            Platform::PsVita => "PS Vita",
            Platform::Genesis => "Mega Drive / Genesis",
            Platform::MasterSystem => "Master System",
            Platform::GameGear => "Game Gear",
            Platform::Saturn => "Saturn",
            Platform::Dreamcast => "Dreamcast",
            Platform::Atari2600 => "Atari 2600",
            Platform::NeoGeo => "Neo Geo",
            Platform::Arcade => "Arcade",
            Platform::Other => "Other",
        }
    }
}

/// A single entry in the user's library. Stable across re-scans because
/// `id` is derived from the file content shape, not its filesystem path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    /// blake/sha-prefixed stable id: hash(canonical_stem + size_bytes).
    pub id: String,
    pub path: PathBuf,
    pub file_name: String,
    pub stem: String,
    pub extension: String,
    pub size_bytes: u64,
    pub modified: DateTime<Utc>,
    pub platform: Platform,
    /// IGDB enrichment, populated by Phase 2.3.
    #[serde(default)]
    pub igdb: Option<IgdbMetadata>,
    /// Local path of a cached cover image, populated by Phase 2.3.
    #[serde(default)]
    pub cover_local_path: Option<PathBuf>,
    #[serde(default)]
    pub last_enriched: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IgdbMetadata {
    pub igdb_id: u64,
    pub name: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub cover_url: Option<String>,
    #[serde(default)]
    pub release_year: Option<u16>,
    #[serde(default)]
    pub total_rating: Option<f64>,
    #[serde(default)]
    pub platforms: Vec<String>,
}

/// Wire format sent back from `scan_library` once the walk completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanReport {
    pub roots: Vec<PathBuf>,
    pub total_files_seen: u64,
    pub games_found: usize,
    pub games_new: usize,
    pub games_kept: usize,
    pub elapsed_ms: u64,
}

/// Progress event emitted while a scan runs. Frontend listens via
/// `@tauri-apps/api/event` -> `abyss://library/scan-progress`.
#[derive(Debug, Clone, Serialize)]
pub struct ScanProgressEvent {
    pub root: PathBuf,
    pub files_seen: u64,
    pub games_found: u64,
    pub current_file: Option<String>,
}

/// Persisted configuration. Backed by `tauri-plugin-store` under the key
/// [`super::config::LIBRARY_CONFIG_KEY`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryConfig {
    #[serde(default)]
    pub scan_paths: Vec<PathBuf>,
    /// IGDB / Twitch client credentials. Stored locally only — never
    /// uploaded anywhere by the app. Populated via Settings UI.
    #[serde(default)]
    pub igdb_client_id: Option<String>,
    #[serde(default)]
    pub igdb_client_secret: Option<String>,
    /// First-run wizard completion timestamp. When `None`, the React side
    /// shows the onboarding overlay on launch; set to `Some(now)` after
    /// the user finishes (or explicitly skips) the wizard.
    #[serde(default)]
    pub wizard_completed_at: Option<DateTime<Utc>>,
}
