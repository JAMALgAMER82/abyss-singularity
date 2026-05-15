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
/// Map a library platform to the libretro core DLL filename that should
/// launch it via RetroArch. `None` means "no specific core, let RetroArch
/// show its menu" — useful for platforms RetroArch doesn't ship a core
/// for in the stable bundle. Filenames match the layout in
/// `RetroArch-Win64/cores/`.
pub fn retroarch_core_for(platform: crate::library::types::Platform) -> Option<&'static str> {
    use crate::library::types::Platform::*;
    Some(match platform {
        Nes             => "nestopia_libretro.dll",
        Snes            => "snes9x_libretro.dll",
        N64             => "mupen64plus_next_libretro.dll",
        Gameboy
            | GameboyColor => "gambatte_libretro.dll",
        GameboyAdvance  => "mgba_libretro.dll",
        Nds             => "desmume_libretro.dll",
        // Genesis Plus GX is the single best Sega core — covers Mega
        // Drive / Genesis, Master System, Game Gear, and SG-1000 from
        // one .dll, with the strongest accuracy + bios-free profile.
        Genesis
            | MasterSystem
            | GameGear      => "genesis_plus_gx_libretro.dll",
        Ps1             => "swanstation_libretro.dll",
        Psp             => "ppsspp_libretro.dll",
        Atari2600       => "stella_libretro.dll",
        NeoGeo
            | Arcade        => "fbneo_libretro.dll",
        Saturn          => "mednafen_saturn_libretro.dll",
        Dreamcast       => "flycast_libretro.dll",
        // The rest fall through to "let RetroArch pick" — these
        // platforms have dedicated standalones that own them in
        // Abyss's default assignments.
        _ => return None,
    })
}

pub fn is_embeddable(_emulator_id: &str) -> bool {
    // Win32 SetParent breaks rendering for every GPU-accelerated
    // emulator we ship: DXGI/D3D11 swap chains and Vulkan/OpenGL
    // contexts bind to the original top-level HWND at creation, so
    // reparenting orphans the present target — game audio plays but
    // the framebuffer never reaches the embedded surface (black
    // window). The orchestration::commands launch path falls back to
    // "minimise Abyss, run emulator as its own window, restore on
    // exit" instead, which works for everything we wrap.
    false
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
        // Ryujinx removed: project taken down October 2024; legal status
        // unstable enough we don't want to ship an install path.

        // PCSX2 Qt v2.x uses single-dash flags; `-batch` skips the GUI
        // prompt and `--` separates options from the positional ROM path.
        r("pcsx2", "PCSX2 (PS2)",
            &["-batch", "-fullscreen", "--", "{game_path}"],
            &[Platform::Ps2]),
        // RPCS3 boots a disc/iso via --no-gui; --fullscreen would be
        // nice but the build complains, so leave it off — it remembers
        // last-used window size and fullscreen toggle (Ctrl+F11) anyway.
        r("rpcs3", "RPCS3 (PS3)",
            &["--no-gui", "{game_path}"],
            &[Platform::Ps3]),
        // DuckStation is the "pcsx-redux" id below (the original PCSX-Redux
        // releases dried up so we repurposed its slot). No separate
        // "duckstation" recipe to avoid a duplicate Settings tile.
        r("ppsspp", "PPSSPP (PSP)",
            &["--fullscreen", "{game_path}"],
            &[Platform::Psp]),

        r("mgba", "mGBA (GBA)",
            &["-f", "{game_path}"],
            &[Platform::GameboyAdvance]),
        r("desmume", "DeSmuME (DS)",
            &["--fullscreen", "{game_path}"],
            &[Platform::Nds]),
        // Citra removed: project shut down November 2024. Active forks
        // (Lime3DS, Azahar) are legally untested — leaving 3DS to user
        // configuration for now.

        r("flycast", "Flycast (Dreamcast)",
            &["-config", "window:fullscreen=yes", "{game_path}"],
            &[Platform::Dreamcast]),
        // Mednafen removed: every system it covers is already handled by
        // RetroArch with stronger controller defaults and a smaller setup
        // surface. One fewer redundant tile.

        // Standalone alternatives for users who want a non-libretro path.
        // Snes9x 1.62.3's CLI is finicky — `-fullscreen` triggers an
        // access violation on launch (verified 2026-05-14). Bare-ROM
        // works fine; the user can press Alt+Enter in-game to toggle
        // fullscreen.
        r("snes9x", "Snes9x (SNES)",
            &["{game_path}"],
            &[Platform::Snes]),
        // The "project64" id is now backed by Simple64 (Project64's repo
        // dropped all releases). Simple64's CLI takes the ROM path
        // positionally; the original args still work.
        r("project64", "Simple64 (N64)",
            &["{game_path}"],
            &[Platform::N64]),
        // The "pcsx-redux" id is now backed by DuckStation. `-fastboot`
        // tells DuckStation to skip the PS1 BIOS entirely (uses HLE);
        // works for ~95% of commercial titles without the user having to
        // dump their own scph1001.bin. `-fullscreen <rom>` then auto-boots.
        r("pcsx-redux", "DuckStation (PS1)",
            &["-fastboot", "-fullscreen", "{game_path}"],
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
