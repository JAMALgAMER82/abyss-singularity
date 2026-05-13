//! Hardcoded set of emulator install manifests.
//!
//! Sources:
//!   * Each URL points at an *official* upstream artifact (project
//!     homepage, GitHub release, or the project's first-party CDN).
//!   * Pinned versions where possible — when a project ships a
//!     stable "latest" channel (RPCS3, PPSSPP) we use that.
//!
//! License notes only — all entries here are GPL / MPL / similar.
//! We do *not* redistribute proprietary BIOS images, save data, or
//! commercial ROMs.

use crate::library::types::Platform;

use super::types::{ArchiveFormat, EmulatorManifest};

pub fn all() -> Vec<EmulatorManifest> {
    vec![
        // RetroArch — the universal libretro frontend. Covers most retro
        // systems via swappable cores; pairs especially well with the
        // SetParent embedder because its window is a single, stable HWND.
        EmulatorManifest {
            id:             "retroarch".into(),
            name:           "RetroArch (multi-system)".into(),
            homepage:       "https://www.retroarch.com/".into(),
            license:        "GPLv3".into(),
            url:            "https://buildbot.libretro.com/stable/1.20.0/windows/x86_64/RetroArch.7z".into(),
            archive_format: ArchiveFormat::SevenZ,
            exe_relpath:    "RetroArch-Win64/retroarch.exe".into(),
            platforms:      vec![
                Platform::Nes, Platform::Snes, Platform::N64, Platform::Gameboy,
                Platform::GameboyColor, Platform::GameboyAdvance, Platform::Nds,
                Platform::Genesis, Platform::MasterSystem, Platform::GameGear,
                Platform::Ps1, Platform::Psp, Platform::Atari2600, Platform::NeoGeo,
                Platform::Arcade, Platform::Saturn, Platform::Dreamcast,
            ],
            approx_size_mb: 90,
            embeddable:     true,
        },

        // Dolphin — GameCube + Wii (Wii U has a separate path).
        EmulatorManifest {
            id:             "dolphin".into(),
            name:           "Dolphin (GameCube / Wii)".into(),
            homepage:       "https://dolphin-emu.org/".into(),
            license:        "GPLv2".into(),
            url:            "https://dl.dolphin-emu.org/releases/2412/dolphin-2412-x64.7z".into(),
            archive_format: ArchiveFormat::SevenZ,
            exe_relpath:    "Dolphin-x64/Dolphin.exe".into(),
            platforms:      vec![Platform::GameCube, Platform::Wii],
            approx_size_mb: 70,
            embeddable:     false,
        },

        // PCSX2 v2.4+ (Qt) — modern PS2. Qt UI doesn't reparent cleanly.
        EmulatorManifest {
            id:             "pcsx2".into(),
            name:           "PCSX2 (PlayStation 2)".into(),
            homepage:       "https://pcsx2.net/".into(),
            license:        "GPLv3".into(),
            url:            "https://github.com/PCSX2/pcsx2/releases/download/v2.4.0/pcsx2-v2.4.0-windows-x64-Qt.7z".into(),
            archive_format: ArchiveFormat::SevenZ,
            exe_relpath:    "pcsx2-qt.exe".into(),
            platforms:      vec![Platform::Ps2],
            approx_size_mb: 150,
            embeddable:     false,
        },

        // RPCS3 — PS3. Continuously released, single rolling URL.
        EmulatorManifest {
            id:             "rpcs3".into(),
            name:           "RPCS3 (PlayStation 3)".into(),
            homepage:       "https://rpcs3.net/".into(),
            license:        "GPLv2".into(),
            url:            "https://github.com/RPCS3/rpcs3-binaries-win/releases/latest/download/rpcs3.7z".into(),
            archive_format: ArchiveFormat::SevenZ,
            exe_relpath:    "rpcs3.exe".into(),
            platforms:      vec![Platform::Ps3],
            approx_size_mb: 130,
            embeddable:     false,
        },

        // PPSSPP — PSP. Lightweight, embeds reasonably.
        EmulatorManifest {
            id:             "ppsspp".into(),
            name:           "PPSSPP (PSP)".into(),
            homepage:       "https://www.ppsspp.org/".into(),
            license:        "GPLv2".into(),
            url:            "https://www.ppsspp.org/files/1_17_1/ppsspp_win.zip".into(),
            archive_format: ArchiveFormat::Zip,
            exe_relpath:    "PPSSPPWindows64.exe".into(),
            platforms:      vec![Platform::Psp],
            approx_size_mb: 30,
            embeddable:     true,
        },

        // mGBA — GBA. Mature, stable Win32 UI, embeds well.
        EmulatorManifest {
            id:             "mgba".into(),
            name:           "mGBA (Game Boy Advance)".into(),
            homepage:       "https://mgba.io/".into(),
            license:        "MPL-2.0".into(),
            url:            "https://github.com/mgba-emu/mgba/releases/download/0.10.5/mGBA-0.10.5-win64.7z".into(),
            archive_format: ArchiveFormat::SevenZ,
            exe_relpath:    "mGBA-0.10.5-win64/mGBA.exe".into(),
            platforms:      vec![Platform::GameboyAdvance, Platform::Gameboy, Platform::GameboyColor],
            approx_size_mb: 15,
            embeddable:     true,
        },

        // DeSmuME — DS. Classic Win32 UI.
        EmulatorManifest {
            id:             "desmume".into(),
            name:           "DeSmuME (Nintendo DS)".into(),
            homepage:       "https://desmume.org/".into(),
            license:        "GPLv2".into(),
            url:            "https://github.com/TASEmulators/desmume/releases/download/release_0_9_13/desmume-0.9.13-win64.zip".into(),
            archive_format: ArchiveFormat::Zip,
            exe_relpath:    "DeSmuME_0.9.13_x64.exe".into(),
            platforms:      vec![Platform::Nds],
            approx_size_mb: 8,
            embeddable:     true,
        },

        // Cemu — Wii U. Open-sourced 2022, actively developed.
        EmulatorManifest {
            id:             "cemu".into(),
            name:           "Cemu (Wii U)".into(),
            homepage:       "https://cemu.info/".into(),
            license:        "MPL-2.0".into(),
            url:            "https://github.com/cemu-project/Cemu/releases/download/v2.0-90/cemu-2.0-90-windows-x64.zip".into(),
            archive_format: ArchiveFormat::Zip,
            exe_relpath:    "Cemu.exe".into(),
            platforms:      vec![Platform::WiiU],
            approx_size_mb: 50,
            embeddable:     false,
        },

        // Snes9x — high-compat standalone SNES. Pairs well with RetroArch.
        EmulatorManifest {
            id:             "snes9x".into(),
            name:           "Snes9x (SNES standalone)".into(),
            homepage:       "https://www.snes9x.com/".into(),
            license:        "Permissive (custom)".into(),
            url:            "https://github.com/snes9xgit/snes9x/releases/download/1.62.3/snes9x-1.62.3-win32-x64.zip".into(),
            archive_format: ArchiveFormat::Zip,
            exe_relpath:    "snes9x-x64.exe".into(),
            platforms:      vec![Platform::Snes],
            approx_size_mb: 5,
            embeddable:     true,
        },

        // Project64 — long-running N64 emulator.
        EmulatorManifest {
            id:             "project64".into(),
            name:           "Project64 (Nintendo 64)".into(),
            homepage:       "https://www.pj64-emu.com/".into(),
            license:        "GPLv2".into(),
            url:            "https://github.com/project64/project64/releases/download/3.0.1/Project64-3.0.1-7-c5c.zip".into(),
            archive_format: ArchiveFormat::Zip,
            exe_relpath:    "Project64.exe".into(),
            platforms:      vec![Platform::N64],
            approx_size_mb: 20,
            embeddable:     true,
        },

        // PCSX-Redux — modern PS1 alternative to DuckStation.
        EmulatorManifest {
            id:             "pcsx-redux".into(),
            name:           "PCSX-Redux (PlayStation)".into(),
            homepage:       "https://pcsx-redux.consoledev.net/".into(),
            license:        "GPLv2".into(),
            url:            "https://github.com/grumpycoders/pcsx-redux/releases/download/v25.04.0/pcsx-redux-windows-x86_64.zip".into(),
            archive_format: ArchiveFormat::Zip,
            exe_relpath:    "pcsx-redux.exe".into(),
            platforms:      vec![Platform::Ps1],
            approx_size_mb: 25,
            embeddable:     false,
        },

        // Stella — Atari 2600.
        EmulatorManifest {
            id:             "stella".into(),
            name:           "Stella (Atari 2600)".into(),
            homepage:       "https://stella-emu.github.io/".into(),
            license:        "GPLv2".into(),
            url:            "https://github.com/stella-emu/stella/releases/download/7.0/Stella-7.0-x64.zip".into(),
            archive_format: ArchiveFormat::Zip,
            exe_relpath:    "Stella-7.0/64-bit/Stella.exe".into(),
            platforms:      vec![Platform::Atari2600],
            approx_size_mb: 12,
            embeddable:     true,
        },
    ]
}

pub fn find_by_id(id: &str) -> Option<EmulatorManifest> {
    all().into_iter().find(|m| m.id == id)
}
