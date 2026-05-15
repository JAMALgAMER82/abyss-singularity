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
        // PC — direct launch
        ("exe",  Pc),
        ("lnk",  Pc),
        ("url",  Pc),
        ("bat",  Pc),
        ("cmd",  Pc),
        ("msi",  Pc),
        // Nintendo — NES family
        ("nes",  Nes),
        ("unf",  Nes),
        ("unif", Nes),
        ("fds",  Nes),   // Famicom Disk System
        ("nez",  Nes),
        // Nintendo — SNES family
        ("sfc",  Snes),
        ("smc",  Snes),
        ("swc",  Snes),
        ("fig",  Snes),
        ("bs",   Snes),  // BS-X Satellaview
        // Nintendo — N64
        ("n64",  N64),
        ("z64",  N64),
        ("v64",  N64),
        ("u1",   N64),
        ("ndd",  N64),   // 64DD disk
        // Nintendo — GameCube
        ("gcm",  GameCube),
        ("gcz",  GameCube),
        ("rvz",  GameCube),
        ("ciso", GameCube),
        // Nintendo — Wii
        ("wbfs", Wii),
        ("wad",  Wii),
        ("wia",  Wii),
        // Nintendo — Wii U
        ("wux",  WiiU),
        ("wud",  WiiU),
        ("rpx",  WiiU),
        // Nintendo — Switch
        ("nsp",  Switch),
        ("xci",  Switch),
        ("nca",  Switch),
        ("nro",  Switch),
        ("nsz",  Switch),
        ("xcz",  Switch),
        // Game Boy / Color / Advance / DS / 3DS
        ("gb",   Gameboy),
        ("gbc",  GameboyColor),
        ("gba",  GameboyAdvance),
        ("agb",  GameboyAdvance),
        ("nds",  Nds),
        ("srl",  Nds),
        ("dsi",  Nds),
        ("ids",  Nds),
        ("3ds",  Threeds),
        ("3dsx", Threeds),
        ("cci",  Threeds),
        ("cxi",  Threeds),
        ("cia",  Threeds),
        ("app",  Threeds),
        // Sony — PSP / PS Vita
        ("pbp",  Psp),
        ("cso",  Psp),
        ("psp",  Psp),
        ("prx",  Psp),
        ("vpk",  PsVita),
        ("nps",  PsVita),
        // Sega — Genesis / Master System / Game Gear / Dreamcast / Saturn
        ("md",   Genesis),
        ("gen",  Genesis),
        ("smd",  Genesis),
        ("mdx",  Genesis),
        ("32x",  Genesis),  // Sega 32X — Genesis Plus GX core handles
        ("sms",  MasterSystem),
        ("sg",   MasterSystem),  // SG-1000
        ("gg",   GameGear),
        ("cdi",  Dreamcast),
        ("gdi",  Dreamcast),
        ("dat",  Dreamcast),
        ("ss",   Saturn),
        ("st",   Saturn),
        // Atari
        ("a26",  Atari2600),
        ("a52",  Atari2600),
        ("a78",  Atari2600),
        // Neo Geo / Arcade
        ("neo",  NeoGeo),
        // Ambiguous container formats — heuristic-classified below.
        ("iso",  Other),
        ("bin",  Other),
        ("img",  Other),
        ("chd",  Other),
        ("cue",  Other),
        ("ccd",  Other),
        ("mds",  Other),
        ("mdf",  Other),
        ("nrg",  Other),
        ("m3u",  Other),   // multi-disc playlist
        ("pkg",  Other),   // PSP/PS3/Switch package
        ("ecm",  Other),   // Error Code Modeler — compressed disc image (mostly PS1)
        ("toc",  Other),   // CDRWIN table-of-contents (Saturn/PS1)
        ("zip",  Other),
        ("7z",   Other),
        ("rar",  Other),
    ];

    pairs.iter().copied().collect()
}

/// Refine an `Other` classification using three signals, in order of
/// confidence:
///
/// 1. Parent-directory names — `/PS2/Game.iso` → `Ps2`
/// 2. Filename keywords — known game titles that pin a single platform
///    (e.g. "Tekken 5" is PS2-only, "God of War: Chains of Olympus" is PSP)
/// 3. File size — PSP UMDs cap at ~1.8 GB; PS2/Wii DVDs sit at 4-5 GB;
///    a small ambiguous .iso is almost always PSP, a large one PS2.
///
/// `file_name` is the leaf basename (with extension); `size_bytes` is the
/// file size as observed on disk. Pass `0` if size is unknown — only the
/// path + name signals will run.
pub fn refine_ambiguous(
    initial: Platform,
    path: &std::path::Path,
    file_name: &str,
    size_bytes: u64,
) -> Platform {
    if initial != Platform::Other {
        return initial;
    }
    let lower_path: String = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
        .to_lowercase();

    // 1. Parent-directory tokens — order matters, specific first.
    for (needle, platform) in [
        ("ps3",            Platform::Ps3),
        ("ps2",            Platform::Ps2),
        ("ps1",            Platform::Ps1),
        ("psx",            Platform::Ps1),
        ("psp",            Platform::Psp),
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
        if lower_path.contains(needle) {
            return platform;
        }
    }

    // 2. Filename keyword pins — game titles whose .iso could only be one
    //    platform. Curated, not exhaustive; manual override exists as a
    //    fallback for everything else.
    let lower_name = file_name.to_lowercase();
    for (needle, platform) in [
        // Tekken series
        ("tekken 5",        Platform::Ps2),
        ("tekken 6",        Platform::Ps3),
        ("tekken 4",        Platform::Ps2),
        ("tekken tag",      Platform::Ps2),
        // God of War
        ("god of war iii",                  Platform::Ps3),
        ("god of war 3",                    Platform::Ps3),
        ("god of war ii",                   Platform::Ps2),
        ("god of war 2",                    Platform::Ps2),
        ("god of war: chains of olympus",   Platform::Psp),
        ("god of war: ghost of sparta",     Platform::Psp),
        ("god of war",                      Platform::Ps2),
        // GTA
        ("gta sa", Platform::Ps2),
        ("gta vc", Platform::Ps2),
        ("gta iii", Platform::Ps2),
        ("grand theft auto: san andreas", Platform::Ps2),
        ("grand theft auto: vice city",   Platform::Ps2),
        ("grand theft auto iii",          Platform::Ps2),
        ("grand theft auto: liberty city stories", Platform::Psp),
        ("grand theft auto: vice city stories",    Platform::Psp),
        // Final Fantasy
        ("final fantasy x",   Platform::Ps2),
        ("final fantasy xii", Platform::Ps2),
        // Metal Gear
        ("metal gear solid 2",     Platform::Ps2),
        ("metal gear solid 3",     Platform::Ps2),
        ("metal gear solid 4",     Platform::Ps3),
        ("metal gear solid: peace walker", Platform::Psp),
        // Resident Evil
        ("resident evil 4",  Platform::Ps2),
        ("resident evil 5",  Platform::Ps3),
        // Crash / Spyro
        ("crash twinsanity", Platform::Ps2),
        ("spyro: enter",     Platform::Ps2),
        // Twisted Metal — 1-4 are PS1; "Black" / "Head On" are PS2; "2012" is PS3
        ("twisted metal 2012", Platform::Ps3),
        ("twisted metal black", Platform::Ps2),
        ("twisted metal head on", Platform::Psp),
        ("twisted metal 1", Platform::Ps1),
        ("twisted metal 2", Platform::Ps1),
        ("twisted metal 3", Platform::Ps1),
        ("twisted metal 4", Platform::Ps1),
        // Wii U & Switch hints
        ("breath of the wild",   Platform::Switch),
        ("super mario odyssey",  Platform::Switch),
    ] {
        if lower_name.contains(needle) {
            return platform;
        }
    }

    // 2b. PS1 disc-image serial prefixes — every retail PSX disc carries a
    //     region code: SCUS/SLUS (US), SCES/SLES/SCED (EU), SCPS/SLPS/SLPM
    //     (JP), SCAJ/SCAS (Asia), PBPX/PCPX/PAPX (PocketStation/promo). One
    //     of these in the filename is a near-certain PS1 marker.
    for prefix in [
        "scus-", "slus-", "scus_", "slus_",
        "sces-", "sles-", "sced-", "sces_", "sles_",
        "scps-", "slps-", "slpm-", "scps_", "slps_", "slpm_",
        "scaj-", "scas-", "papx-", "pbpx-", "pcpx-",
    ] {
        if lower_name.contains(prefix) {
            return Platform::Ps1;
        }
    }

    // 3. Descriptor files (.cue / .gdi / .toc / .ccd / .m3u) are pure
    //    pointers — the actual disc bytes live in sibling .bin files,
    //    and the descriptor itself is ~1 KB so the size heuristic is
    //    useless. Use the ecosystem the descriptor format implies:
    //    .cue is overwhelmingly PS1 in retro libraries (TurboCD/Saturn
    //    use the same format but PS1 dominates the dumping scene),
    //    .gdi is Dreamcast-only, and .toc is Saturn-leaning.
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());
    match ext.as_deref() {
        Some("cue") => return Platform::Ps1,
        Some("gdi") => return Platform::Dreamcast,
        Some("toc") => return Platform::Saturn,
        _ => {}
    }

    // 4. Size heuristic for .iso/.bin/.img — refined PS1/PSP/PS2/Wii/PS3
    //    ranges by physical disc geometry:
    //    - PS1 CD: up to ~750 MB
    //    - PSP UMD: 100 MB – 1.8 GB
    //    - PS2 DVD5: ~800 MB – 4.7 GB
    //    - Wii single-layer DVD: ~4.4 GB; dual-layer max ~8.5 GB
    //    - PS3 Blu-ray: 4.7 – 50+ GB
    //    The 4.5 – 8.5 GB band genuinely overlaps Wii dual-layer and PS3
    //    single-layer; we lean Wii because Wii is the more common
    //    retro-library platform at that size. ≥ 8.5 GB is unambiguously
    //    PS3 (above Wii's physical max).
    if matches!(ext.as_deref(), Some("iso") | Some("bin") | Some("img")) {
        const MIB: u64 = 1024 * 1024;
        match size_bytes {
            0                                                 => {}                          // unknown
            sz if sz <=  750  * MIB                            => return Platform::Ps1,
            sz if sz <= 1_800 * MIB                            => return Platform::Psp,
            sz if (1_800 * MIB.. 4_500 * MIB).contains(&sz)    => return Platform::Ps2,
            sz if (4_500 * MIB.. 8_700 * MIB).contains(&sz)    => return Platform::Wii,
            sz if sz >= 8_700 * MIB                            => return Platform::Ps3,
            _ => {}
        }
    }

    Platform::Other
}
