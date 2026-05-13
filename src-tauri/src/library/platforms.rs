//! File-extension → platform mapping.
//!
//! Authoritative for "which file is a game and what runs it." Phase 3
//! (emulator orchestration) will read [`Platform`] off a [`LibraryEntry`]
//! and look up the right emulator binary + arg recipe in a separate
//! `emulators.json` config — keeping platform identity here decoupled
//! from launcher logic.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::types::Platform;

/// Map of lowercase extension (no leading dot) → platform. Returns `None`
/// for files we don't recognise as games.
pub fn platform_for_extension(ext: &str) -> Option<Platform> {
    EXT_MAP.get_or_init(build_ext_map).get(ext).copied()
}

/// Iterator over every (extension, platform) pair we recognise — useful
/// for `walkdir` filters and for the Settings UI to show what gets picked
/// up under each library path.
#[allow(dead_code)] // public API; exercised by tests, future Settings UI will surface this.
pub fn known_extensions() -> impl Iterator<Item = (&'static str, Platform)> {
    EXT_MAP
        .get_or_init(build_ext_map)
        .iter()
        .map(|(k, v)| (*k, *v))
}

static EXT_MAP: OnceLock<HashMap<&'static str, Platform>> = OnceLock::new();

fn build_ext_map() -> HashMap<&'static str, Platform> {
    use Platform::*;
    let pairs: &[(&str, Platform)] = &[
        // PC
        ("exe",  Pc),
        ("lnk",  Pc),
        ("url",  Pc),
        ("bat",  Pc),
        // Nintendo
        ("nes",  Nes),
        ("sfc",  Snes),
        ("smc",  Snes),
        ("n64",  N64),
        ("z64",  N64),
        ("v64",  N64),
        ("gcm",  GameCube),
        ("rvz",  GameCube),
        ("wbfs", Wii),
        ("wad",  Wii),
        ("wux",  WiiU),
        ("nsp",  Switch),
        ("xci",  Switch),
        ("nca",  Switch),
        ("gb",   Gameboy),
        ("gbc",  GameboyColor),
        ("gba",  GameboyAdvance),
        ("nds",  Nds),
        ("3ds",  Threeds),
        ("cia",  Threeds),
        // Sony
        ("pbp",  Psp),
        ("cso",  Psp),
        ("psp",  Psp),
        ("vpk",  PsVita),
        // Sega
        ("md",   Genesis),
        ("gen",  Genesis),
        ("smd",  Genesis),
        ("sms",  MasterSystem),
        ("gg",   GameGear),
        ("cdi",  Dreamcast),
        ("gdi",  Dreamcast),
        // Misc
        ("a26",  Atari2600),
        ("neo",  NeoGeo),
        // Ambiguous container formats — heuristic-classified below.
        ("iso",  Other),
        ("bin",  Other),
        ("img",  Other),
        ("chd",  Other),
        ("cue",  Other),
        ("zip",  Other),
        ("7z",   Other),
    ];

    pairs.iter().copied().collect()
}

/// Refine an `Other` classification by inspecting the parent directory
/// names — `/PS2/Game.iso` → `Ps2`, etc. This is a cheap heuristic; the
/// authoritative match (when we have one) is in the extension map above.
pub fn refine_ambiguous(initial: Platform, path: &std::path::Path) -> Platform {
    if initial != Platform::Other {
        return initial;
    }
    let lower: String = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
        .to_lowercase();

    // Order matters — check the more specific tokens first.
    for (needle, platform) in [
        ("ps3",            Platform::Ps3),
        ("ps2",            Platform::Ps2),
        ("ps1",            Platform::Ps1),
        ("psx",            Platform::Ps1),
        ("playstation 3",  Platform::Ps3),
        ("playstation 2",  Platform::Ps2),
        ("playstation 1",  Platform::Ps1),
        ("playstation",    Platform::Ps1),
        ("dreamcast",      Platform::Dreamcast),
        ("saturn",         Platform::Saturn),
        ("gamecube",       Platform::GameCube),
        ("wii u",          Platform::WiiU),
        ("wii",            Platform::Wii),
        ("switch",         Platform::Switch),
        ("xbox",           Platform::Pc),
        ("arcade",         Platform::Arcade),
        ("mame",           Platform::Arcade),
    ] {
        if lower.contains(needle) {
            return platform;
        }
    }
    Platform::Other
}
