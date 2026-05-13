//! Built-in argument recipes for common emulators.
//!
//! These are *templates* — the user has to supply the actual `.exe` path
//! before they're usable. The Settings UI lets them seed their config from
//! this list and then point each entry at the right binary.
//!
//! Token grammar (case-sensitive, replaced at launch time):
//!   {game_path}  — full absolute path to the ROM/exe being launched
//!   {game_dir}   — parent directory of {game_path}
//!   {game_stem}  — filename without extension

use std::path::PathBuf;

use super::types::EmulatorEntry;
use crate::library::types::Platform;

/// Whitelist of emulator ids whose top-level window reparents cleanly
/// into the Abyss main HWND via Win32 SetParent. Used by the orchestration
/// launch path on Windows to opt-in to the [`super::embed`] flow.
pub fn is_embeddable(emulator_id: &str) -> bool {
    matches!(
        emulator_id,
        "retroarch" | "mgba" | "ppsspp" | "desmume" | "duckstation" | "citra"
            | "mednafen" | "flycast" | "pc-direct" | "snes9x" | "project64" | "stella"
    )
}

/// Returns the canonical built-in recipe list. The user can copy any of
/// these into their config and then set the `exe` field.
pub fn builtin_recipes() -> Vec<EmulatorEntry> {
    // Helper to cut down on the repeated boilerplate.
    fn r(
        id: &str,
        name: &str,
        args: &[&str],
        platforms: &[Platform],
    ) -> EmulatorEntry {
        EmulatorEntry {
            id:          id.to_string(),
            name:        name.to_string(),
            exe:         PathBuf::new(),
            args:        args.iter().map(|s| s.to_string()).collect(),
            working_dir: None,
            env:         Default::default(),
            platforms:   platforms.to_vec(),
        }
    }

    vec![
        // Universal launcher — handles many platforms via swappable cores.
        r("retroarch", "RetroArch (multi-system)",
            &["-f", "{game_path}"],
            &[
                Platform::Nes, Platform::Snes, Platform::N64, Platform::Gameboy,
                Platform::GameboyColor, Platform::GameboyAdvance, Platform::Nds,
                Platform::Genesis, Platform::MasterSystem, Platform::GameGear,
                Platform::Ps1, Platform::Psp, Platform::Atari2600, Platform::NeoGeo,
                Platform::Arcade,
            ]),

        r("dolphin", "Dolphin (GameCube/Wii)",
            &["-b", "-e", "{game_path}"],
            &[Platform::GameCube, Platform::Wii]),
        r("cemu", "Cemu (Wii U)",
            &["-f", "-g", "{game_path}"],
            &[Platform::WiiU]),
        r("ryujinx", "Ryujinx (Switch)",
            &["--fullscreen", "{game_path}"],
            &[Platform::Switch]),

        r("pcsx2", "PCSX2 (PS2)",
            &["--fullscreen", "--", "{game_path}"],
            &[Platform::Ps2]),
        r("rpcs3", "RPCS3 (PS3)",
            &["--no-gui", "{game_path}"],
            &[Platform::Ps3]),
        r("duckstation", "DuckStation (PS1)",
            &["-fullscreen", "{game_path}"],
            &[Platform::Ps1]),
        r("ppsspp", "PPSSPP (PSP)",
            &["--fullscreen", "{game_path}"],
            &[Platform::Psp]),

        r("mgba", "mGBA (GBA)",
            &["-f", "{game_path}"],
            &[Platform::GameboyAdvance]),
        r("desmume", "DeSmuME (DS)",
            &["--fullscreen", "{game_path}"],
            &[Platform::Nds]),
        r("citra", "Citra (3DS)",
            &["{game_path}"],
            &[Platform::Threeds]),

        r("flycast", "Flycast (Dreamcast/Saturn)",
            &["-config", "window:fullscreen=yes", "{game_path}"],
            &[Platform::Dreamcast, Platform::Saturn]),
        r("mednafen", "Mednafen (multi-system)",
            &["-fs", "1", "{game_path}"],
            &[Platform::Genesis, Platform::MasterSystem, Platform::Atari2600, Platform::PsVita]),

        // Standalone alternatives for users who want a non-libretro path.
        r("snes9x", "Snes9x (SNES)",
            &["-fullscreen", "{game_path}"],
            &[Platform::Snes]),
        r("project64", "Project64 (N64)",
            &["{game_path}"],
            &[Platform::N64]),
        r("pcsx-redux", "PCSX-Redux (PS1)",
            &["-iso", "{game_path}", "-run"],
            &[Platform::Ps1]),
        r("stella", "Stella (Atari 2600)",
            &["{game_path}"],
            &[Platform::Atari2600]),

        // PC games: no emulator — launch the binary directly. {game_path}
        // is itself an .exe/.lnk, so the "emulator" is a passthrough.
        r("pc-direct", "PC (direct launch)",
            &[], // args appended after exe = {game_path} would mean self-launch.
            &[Platform::Pc]),
    ]
}

/// Expand placeholder tokens in an argument template using the given
/// library entry. Public for unit testing — the substitution is
/// finicky enough that I want it pinned in tests.
pub fn expand_args(template: &[String], entry: &crate::library::types::LibraryEntry) -> Vec<String> {
    let game_path = entry.path.to_string_lossy().to_string();
    let game_dir = entry
        .path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let game_stem = entry.stem.clone();

    template
        .iter()
        .map(|t| {
            t.replace("{game_path}", &game_path)
                .replace("{game_dir}", &game_dir)
                .replace("{game_stem}", &game_stem)
        })
        .collect()
}
