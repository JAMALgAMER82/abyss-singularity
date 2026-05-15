//! Per-emulator default controller config writers.
//!
//! ## Auto-detect across controller types
//!
//! Users plug in everything: Xbox 360 / One / Series, PS4 DualShock, PS5
//! DualSense, Switch Pro, 8BitDo, generic Logitech, Stadia, etc. The
//! strategy here splits cleanly into two camps:
//!
//! - **SDL-backed emulators** (PCSX2, DuckStation, every libretro core
//!   in RetroArch, PPSSPP, Flycast, mGBA): SDL2 carries an internal
//!   gamepad database that already knows the button maps for all the
//!   pads above. Binding to `SDL-N/Button-X` works for whichever
//!   physical pad gets enumerated as index N — Xbox, PS5, 8BitDo,
//!   doesn't matter. No probing needed.
//!
//! - **XInput-backed emulators** (Dolphin): the XInput API only sees
//!   Xbox-protocol pads. In practice this covers the vast majority of
//!   real-world setups because Steam Input (running on most gaming PCs)
//!   and DS4Windows both present PS4/PS5 controllers to the OS as
//!   XInput gamepads. Plain-native PS pads without Steam/DS4Windows are
//!   the one edge case — those users have to either install DS4Windows
//!   or click through Dolphin's controller dialog once.
//!
//! ## What this module actually writes
//!
//! A one-time *seed* for PCSX2/DuckStation/Dolphin so a fresh Abyss
//! install "just works" the first time the user clicks Play. We never
//! overwrite an existing user config (fingerprint check on the
//! upstream defaults), and we skip silently when files already have
//! non-default bindings.
//!
//! Slots 1-4 are seeded so couch co-op (4 pads on one host) works out
//! of the box. Player 1 retains a keyboard fallback; players 2-4 are
//! gamepad-only (no way to share a keyboard across four humans).
//!
//! ## When this runs
//!
//! - At install time from [`installer::commands::installer_install_all`].
//! - Pre-launch from [`orchestration::commands::orch_launch`] and the
//!   lobby launcher, in case the user wiped their emulator config
//!   between install and play.
//! - On demand from the Diagnose & Repair button.

use std::path::PathBuf;

/// Returns the path PCSX2 reads its config from on Windows. This is
/// `%USERPROFILE%\Documents\PCSX2\inis\PCSX2.ini` unless the user has
/// chosen portable mode (a `portable.ini` next to the exe), which Abyss
/// never does for them. We resolve %USERPROFILE% via env var to avoid
/// pulling in an extra crate.
#[cfg(target_os = "windows")]
fn pcsx2_ini_path() -> Option<PathBuf> {
    let profile = std::env::var_os("USERPROFILE")?;
    Some(PathBuf::from(profile).join("Documents").join("PCSX2").join("inis").join("PCSX2.ini"))
}

#[cfg(not(target_os = "windows"))]
fn pcsx2_ini_path() -> Option<PathBuf> { None }

/// PCSX2 `[PadN]` body. SDL device index is `slot - 1`, so Pad1 = SDL-0,
/// Pad2 = SDL-1, etc. Keyboard fallback is only emitted for slot 1.
fn pcsx2_pad_body(slot: u8) -> String {
    let sdl = slot.saturating_sub(1);
    let kb = slot == 1;
    let or_kb = |key: &str| -> String {
        if kb { format!(" & Keyboard/{key}") } else { String::new() }
    };
    format!(
"Type = DualShock2
InvertL = 0
InvertR = 0
Deadzone = 0
AxisScale = 1.33
LargeMotorScale = 1
SmallMotorScale = 1
ButtonDeadzone = 0
PressureModifier = 0.5
Up = SDL-{sdl}/DPadUp{}
Right = SDL-{sdl}/DPadRight{}
Down = SDL-{sdl}/DPadDown{}
Left = SDL-{sdl}/DPadLeft{}
Triangle = SDL-{sdl}/Y{}
Circle = SDL-{sdl}/B{}
Cross = SDL-{sdl}/A{}
Square = SDL-{sdl}/X{}
Select = SDL-{sdl}/Back{}
Start = SDL-{sdl}/Start{}
L1 = SDL-{sdl}/LeftShoulder{}
L2 = SDL-{sdl}/+LeftTrigger{}
R1 = SDL-{sdl}/RightShoulder{}
R2 = SDL-{sdl}/+RightTrigger{}
L3 = SDL-{sdl}/LeftStick{}
R3 = SDL-{sdl}/RightStick{}
LUp = SDL-{sdl}/-LeftY{}
LRight = SDL-{sdl}/+LeftX{}
LDown = SDL-{sdl}/+LeftY{}
LLeft = SDL-{sdl}/-LeftX{}
RUp = SDL-{sdl}/-RightY{}
RRight = SDL-{sdl}/+RightX{}
RDown = SDL-{sdl}/+RightY{}
RLeft = SDL-{sdl}/-RightX{}
SmallMotor = SDL-{sdl}/SmallMotor
LargeMotor = SDL-{sdl}/LargeMotor",
        or_kb("Up"), or_kb("Right"), or_kb("Down"), or_kb("Left"),
        or_kb("I"), or_kb("L"), or_kb("K"), or_kb("J"),
        or_kb("Backspace"), or_kb("Return"),
        or_kb("Q"), or_kb("1"), or_kb("E"), or_kb("3"), or_kb("2"), or_kb("4"),
        or_kb("W"), or_kb("D"), or_kb("S"), or_kb("A"),
        or_kb("T"), or_kb("H"), or_kb("G"), or_kb("F"),
    )
}

/// "PCSX2 ships with these defaults" — the fingerprint we use to detect
/// an untouched config we're safe to overwrite. Conservative: if any of
/// these are present we assume the user hasn't customised Pad1 yet.
const PCSX2_DEFAULT_KB_BINDINGS: &[&str] = &[
    "Triangle = Keyboard/I",
    "Cross = Keyboard/K",
    "Square = Keyboard/J",
    "Circle = Keyboard/L",
];

/// Seed PCSX2's controller config. Three cases:
///   1. PCSX2.ini doesn't exist → write the full seed (Pad1..Pad4).
///   2. PCSX2.ini exists with the keyboard-only defaults → patch
///      `[InputSources] XInput = true` and replace `[Pad1]` in place,
///      then ensure Pad2..Pad4 are present.
///   3. PCSX2.ini exists with user-customised Pad1 → leave alone.
///
/// Returns `Ok(true)` if we modified anything, `Ok(false)` otherwise.
pub fn ensure_pcsx2_default() -> Result<bool, String> {
    let Some(ini) = pcsx2_ini_path() else { return Ok(false) };

    // Case 1: file doesn't exist → seed all four pads.
    if !ini.exists() {
        if let Some(parent) = ini.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("creating {}: {e}", parent.display()))?;
        }
        let mut seed = String::from(
"[UI]
SettingsVersion = 1
SetupWizardIncomplete = false

[InputSources]
Keyboard = true
Mouse = true
SDL = true
DInput = false
XInput = true
SDLControllerEnhancedMode = true
SDLPS5PlayerLED = true

");
        for slot in 1..=4 {
            seed.push_str(&format!("[Pad{slot}]\n{}\n\n", pcsx2_pad_body(slot)));
        }
        std::fs::write(&ini, seed)
            .map_err(|e| format!("writing {}: {e}", ini.display()))?;
        log::info!("controller_setup: seeded fresh PCSX2.ini with gamepad bindings (Pad1..Pad4)");
        return Ok(true);
    }

    // Case 2/3: file exists — read and decide whether to patch.
    let text = std::fs::read_to_string(&ini)
        .map_err(|e| format!("reading {}: {e}", ini.display()))?;

    // Skip if Pad1 has been customised away from the upstream default.
    let pad1_is_default = PCSX2_DEFAULT_KB_BINDINGS.iter().all(|needle| text.contains(needle));
    if !pad1_is_default {
        return Ok(false);
    }

    let mut text = ensure_input_source(&text, "XInput", "true");
    text = ensure_input_source(&text, "SDL", "true");
    for slot in 1..=4 {
        let header = format!("[Pad{slot}]");
        let body = pcsx2_pad_body(slot);
        text = replace_section(&text, &header, &format!("{header}\n{body}"));
    }
    std::fs::write(&ini, text)
        .map_err(|e| format!("writing {}: {e}", ini.display()))?;
    log::info!("controller_setup: patched default PCSX2.ini → Pad1..Pad4 bound to SDL-0..SDL-3");
    Ok(true)
}

/// Make sure `Key = value` is set inside `[InputSources]`. Inserts the
/// line if missing; replaces it if present with a different value.
fn ensure_input_source(text: &str, key: &str, value: &str) -> String {
    let section_header = "[InputSources]";
    let Some(sec_start) = text.find(section_header) else {
        return format!("{}\n{section_header}\n{key} = {value}\n", text.trim_end());
    };
    let body_start = sec_start + section_header.len();
    let body_end = text[body_start..]
        .find("\n[")
        .map(|off| body_start + off)
        .unwrap_or(text.len());
    let body = &text[body_start..body_end];
    let new_body = upsert_kv_line(body, key, value);
    format!("{}{}{}", &text[..body_start], new_body, &text[body_end..])
}

fn upsert_kv_line(body: &str, key: &str, value: &str) -> String {
    let mut out = String::with_capacity(body.len() + 32);
    let mut replaced = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if let Some(eq_pos) = trimmed.find('=') {
            let lhs = trimmed[..eq_pos].trim();
            if lhs.eq_ignore_ascii_case(key) {
                out.push_str(&format!("{key} = {value}"));
                out.push('\n');
                replaced = true;
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    if !replaced {
        out = format!("\n{key} = {value}\n{}", out.trim_start_matches('\n'));
    } else if body.starts_with('\n') && !out.starts_with('\n') {
        out = format!("\n{out}");
    }
    out
}

/// Replace a whole `[Section]` block (header line through to the line
/// before the next `\n[`, or EOF). Appends at the end if the section
/// is missing.
fn replace_section(text: &str, header: &str, replacement: &str) -> String {
    let Some(start) = text.find(header) else {
        return format!("{}\n\n{replacement}\n", text.trim_end());
    };
    let after_header = start + header.len();
    let end = text[after_header..]
        .find("\n[")
        .map(|off| after_header + off)
        .unwrap_or(text.len());
    format!("{}{}{}", &text[..start], replacement, &text[end..])
}

/// DuckStation Pad-N analog controller body bound to SDL device `slot-1`.
/// Keyboard fallback is emitted only for slot 1.
fn duckstation_pad_body(slot: u8) -> String {
    let sdl = slot.saturating_sub(1);
    let kb = slot == 1;
    let or_kb = |key: &str| -> String {
        if kb { format!(" & Keyboard/{key}") } else { String::new() }
    };
    format!(
"Type = AnalogController
Up = SDL-{sdl}/DPadUp{}
Down = SDL-{sdl}/DPadDown{}
Left = SDL-{sdl}/DPadLeft{}
Right = SDL-{sdl}/DPadRight{}
Triangle = SDL-{sdl}/Y{}
Circle = SDL-{sdl}/B{}
Cross = SDL-{sdl}/A{}
Square = SDL-{sdl}/X{}
Select = SDL-{sdl}/Back{}
Start = SDL-{sdl}/Start{}
L1 = SDL-{sdl}/LeftShoulder{}
L2 = SDL-{sdl}/+LeftTrigger{}
R1 = SDL-{sdl}/RightShoulder{}
R2 = SDL-{sdl}/+RightTrigger{}
L3 = SDL-{sdl}/LeftStick{}
R3 = SDL-{sdl}/RightStick{}
LLeftRight = SDL-{sdl}/LeftX
LUpDown = SDL-{sdl}/LeftY
RLeftRight = SDL-{sdl}/RightX
RUpDown = SDL-{sdl}/RightY
SmallMotor = SDL-{sdl}/SmallMotor
LargeMotor = SDL-{sdl}/LargeMotor",
        or_kb("W"), or_kb("S"), or_kb("A"), or_kb("D"),
        or_kb("I"), or_kb("L"), or_kb("K"), or_kb("J"),
        or_kb("Backspace"), or_kb("Return"),
        or_kb("Q"), or_kb("1"), or_kb("E"), or_kb("3"), or_kb("2"), or_kb("4"),
    )
}

#[cfg(target_os = "windows")]
fn duckstation_ini_path() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(local).join("DuckStation").join("settings.ini"))
}

#[cfg(not(target_os = "windows"))]
fn duckstation_ini_path() -> Option<PathBuf> { None }

/// Seed DuckStation Pad1..Pad4. Idempotent: only writes when settings.ini
/// is fresh or has only the default empty/keyboard binding.
pub fn ensure_duckstation_default() -> Result<bool, String> {
    let Some(ini) = duckstation_ini_path() else { return Ok(false) };
    if !ini.exists() {
        if let Some(parent) = ini.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("creating {}: {e}", parent.display()))?;
        }
        let mut seed = String::from(
"[InputSources]
SDL = true
XInput = false
DInput = false

");
        for slot in 1..=4 {
            seed.push_str(&format!("[Pad{slot}]\n{}\n\n", duckstation_pad_body(slot)));
        }
        std::fs::write(&ini, seed)
            .map_err(|e| format!("writing {}: {e}", ini.display()))?;
        log::info!("controller_setup: seeded fresh DuckStation settings.ini (Pad1..Pad4)");
        return Ok(true);
    }

    let text = std::fs::read_to_string(&ini)
        .map_err(|e| format!("reading {}: {e}", ini.display()))?;
    // Already has a non-trivial Pad1? Leave alone.
    if text.contains("[Pad1]") && text.contains("SDL-0/") {
        return Ok(false);
    }
    let mut text = ensure_input_source(&text, "SDL", "true");
    for slot in 1..=4 {
        let header = format!("[Pad{slot}]");
        let body = duckstation_pad_body(slot);
        text = replace_section(&text, &header, &format!("{header}\n{body}"));
    }
    std::fs::write(&ini, text)
        .map_err(|e| format!("writing {}: {e}", ini.display()))?;
    log::info!("controller_setup: patched DuckStation settings.ini Pad1..Pad4 → SDL-0..SDL-3");
    Ok(true)
}

/// Dolphin `[GCPadN]` block bound to XInput device `slot-1`.
fn dolphin_gcpad_body(slot: u8) -> String {
    let dev = slot.saturating_sub(1);
    format!(
"Device = XInput/{dev}/Gamepad
Buttons/A = `Button A`
Buttons/B = `Button B`
Buttons/X = `Button X`
Buttons/Y = `Button Y`
Buttons/Z = `Trigger R`
Buttons/Start = `Button Start`
Main Stick/Up = `Left Y+`
Main Stick/Down = `Left Y-`
Main Stick/Left = `Left X-`
Main Stick/Right = `Left X+`
Main Stick/Modifier = `Thumb L`
Main Stick/Modifier/Range = 50.000000
C-Stick/Up = `Right Y+`
C-Stick/Down = `Right Y-`
C-Stick/Left = `Right X-`
C-Stick/Right = `Right X+`
C-Stick/Modifier = `Thumb R`
C-Stick/Modifier/Range = 50.000000
Triggers/L = `Trigger L`
Triggers/R = `Trigger R`
Triggers/L-Analog = `Trigger L`
Triggers/R-Analog = `Trigger R`
D-Pad/Up = `Pad N`
D-Pad/Down = `Pad S`
D-Pad/Left = `Pad W`
D-Pad/Right = `Pad E`
Rumble/Motor = `Motor L`|`Motor R`")
}

#[cfg(target_os = "windows")]
fn dolphin_gcpad_ini_path() -> Option<PathBuf> {
    let profile = std::env::var_os("USERPROFILE")?;
    Some(PathBuf::from(profile)
        .join("Documents")
        .join("Dolphin Emulator")
        .join("Config")
        .join("GCPadNew.ini"))
}

#[cfg(not(target_os = "windows"))]
fn dolphin_gcpad_ini_path() -> Option<PathBuf> { None }

/// Seed Dolphin GCPadNew.ini with working XInput bindings for slots 1-4
/// when none exists. Conservative: if the file is already there at all
/// (even just the empty stub Dolphin writes on its own first launch) we
/// leave it alone — Dolphin's UI rewrites this file frequently and
/// clobbering a user's config would be much worse than the missing
/// initial binding.
pub fn ensure_dolphin_default() -> Result<bool, String> {
    let Some(ini) = dolphin_gcpad_ini_path() else { return Ok(false) };
    if ini.exists() { return Ok(false); }
    if let Some(parent) = ini.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("creating {}: {e}", parent.display()))?;
    }
    let mut seed = String::new();
    for slot in 1..=4 {
        seed.push_str(&format!("[GCPad{slot}]\n{}\n\n", dolphin_gcpad_body(slot)));
    }
    std::fs::write(&ini, seed)
        .map_err(|e| format!("writing {}: {e}", ini.display()))?;
    log::info!("controller_setup: seeded fresh Dolphin GCPadNew.ini for XInput pads 0..3");
    Ok(true)
}

/// Apply every per-emulator default we know about. PCSX2 and DuckStation
/// are the two PlayStation emulators whose first-run config binds Pad 1
/// to keyboard only. Dolphin gets a seed only when GCPadNew.ini is
/// missing entirely — its UI rewrites the file constantly so we never
/// patch it in place. Everything else (PPSSPP, Snes9x, mGBA, DeSmuME,
/// Stella, Simple64, Flycast, RPCS3) uses SDL2 directly and auto-detects
/// gamepads on hot-plug without intervention.
///
/// Idempotent and cheap to call repeatedly — also invoked pre-launch by
/// [`orch_launch`] and the lobby launcher so a user who wipes their
/// emulator config between install and play still gets a working seed.
pub fn apply_all_defaults() -> Vec<String> {
    let mut applied = Vec::new();
    match ensure_pcsx2_default() {
        Ok(true)  => applied.push("pcsx2".to_string()),
        Ok(false) => {}
        Err(e)    => log::warn!("controller_setup: pcsx2 seed failed: {e}"),
    }
    match ensure_duckstation_default() {
        Ok(true)  => applied.push("duckstation".to_string()),
        Ok(false) => {}
        Err(e)    => log::warn!("controller_setup: duckstation seed failed: {e}"),
    }
    match ensure_dolphin_default() {
        Ok(true)  => applied.push("dolphin".to_string()),
        Ok(false) => {}
        Err(e)    => log::warn!("controller_setup: dolphin seed failed: {e}"),
    }
    applied
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_input_source_replaces_existing() {
        let ini = "[InputSources]\nKeyboard = true\nXInput = false\n\n[Other]\nx = 1\n";
        let out = ensure_input_source(ini, "XInput", "true");
        assert!(out.contains("XInput = true"));
        assert!(!out.contains("XInput = false"));
        assert!(out.contains("[Other]"));
        assert!(out.contains("x = 1"));
    }

    #[test]
    fn ensure_input_source_adds_section_when_missing() {
        let ini = "[Foo]\na = 1\n";
        let out = ensure_input_source(ini, "XInput", "true");
        assert!(out.contains("[InputSources]"));
        assert!(out.contains("XInput = true"));
    }

    #[test]
    fn replace_section_swaps_pad1_block() {
        let ini = "[InputSources]\nSDL = true\n\n[Pad1]\nType = DualShock2\nUp = Keyboard/Up\n\n[Pad2]\nType = None\n";
        let out = replace_section(ini, "[Pad1]", "[Pad1]\nType = DualShock2\nUp = SDL-0/DPadUp");
        assert!(out.contains("Up = SDL-0/DPadUp"));
        assert!(!out.contains("Up = Keyboard/Up"));
        assert!(out.contains("[Pad2]"));
    }

    #[test]
    fn replace_section_appends_when_missing() {
        let ini = "[InputSources]\nSDL = true\n";
        let out = replace_section(ini, "[Pad1]", "[Pad1]\nType = DualShock2");
        assert!(out.contains("[Pad1]"));
        assert!(out.contains("Type = DualShock2"));
    }

    #[test]
    fn pcsx2_pad_body_binds_to_correct_sdl_slot() {
        for (slot, expected) in [(1u8, "SDL-0/"), (2, "SDL-1/"), (3, "SDL-2/"), (4, "SDL-3/")] {
            let body = pcsx2_pad_body(slot);
            assert!(body.contains(expected), "Pad{slot} should reference {expected}, got:\n{body}");
        }
    }

    #[test]
    fn pcsx2_pad_body_keyboard_fallback_only_for_slot_1() {
        assert!(pcsx2_pad_body(1).contains("Keyboard/"));
        assert!(!pcsx2_pad_body(2).contains("Keyboard/"));
        assert!(!pcsx2_pad_body(3).contains("Keyboard/"));
        assert!(!pcsx2_pad_body(4).contains("Keyboard/"));
    }

    #[test]
    fn duckstation_pad_body_binds_to_correct_sdl_slot() {
        for (slot, expected) in [(1u8, "SDL-0/"), (2, "SDL-1/"), (3, "SDL-2/"), (4, "SDL-3/")] {
            let body = duckstation_pad_body(slot);
            assert!(body.contains(expected), "Pad{slot} should reference {expected}");
        }
    }

    #[test]
    fn dolphin_gcpad_body_binds_to_correct_xinput_slot() {
        for (slot, expected) in [(1u8, "XInput/0/"), (2, "XInput/1/"), (3, "XInput/2/"), (4, "XInput/3/")] {
            let body = dolphin_gcpad_body(slot);
            assert!(body.contains(expected), "GCPad{slot} should reference {expected}");
            assert!(body.contains("Buttons/A"));
            assert!(body.contains("Rumble/Motor"));
        }
    }
}
