//! Per-emulator default controller config writers.
//!
//! Most emulators we ship (RetroArch, PPSSPP, Snes9x, mGBA, DeSmuME,
//! Dolphin via SDL backend) auto-detect XInput controllers and bind a
//! sensible default profile on first launch. PCSX2 v2 is the exception
//! — its first-run config has keyboard-only bindings and the
//! `[InputSources]` block disables XInput, so an Xbox/PS pad simply
//! doesn't register until the user opens Settings → Controllers and
//! manually assigns each button.
//!
//! This module writes a one-time default for PCSX2 (and any other
//! emulators we discover need the same treatment) so a freshly-installed
//! Abyss "just works" with a controller the first time the user clicks
//! Play. It's a *seed*: if the user already has a config we never
//! overwrite — they may have customised their bindings.

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

/// Minimal PCSX2.ini that enables XInput input source and binds Pad 1 to
/// SDL gamepad 0 with standard Xbox→PS2 button mapping, keeping the
/// keyboard as a fallback. Everything else PCSX2 fills in with safe
/// defaults on first launch.
const PCSX2_INI_SEED: &str = r#"[UI]
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

[Pad1]
Type = DualShock2
InvertL = 0
InvertR = 0
Deadzone = 0
AxisScale = 1.33
LargeMotorScale = 1
SmallMotorScale = 1
ButtonDeadzone = 0
PressureModifier = 0.5
Up = SDL-0/DPadUp & Keyboard/Up
Right = SDL-0/DPadRight & Keyboard/Right
Down = SDL-0/DPadDown & Keyboard/Down
Left = SDL-0/DPadLeft & Keyboard/Left
Triangle = SDL-0/Y & Keyboard/I
Circle = SDL-0/B & Keyboard/L
Cross = SDL-0/A & Keyboard/K
Square = SDL-0/X & Keyboard/J
Select = SDL-0/Back & Keyboard/Backspace
Start = SDL-0/Start & Keyboard/Return
L1 = SDL-0/LeftShoulder & Keyboard/Q
L2 = SDL-0/+LeftTrigger & Keyboard/1
R1 = SDL-0/RightShoulder & Keyboard/E
R2 = SDL-0/+RightTrigger & Keyboard/3
L3 = SDL-0/LeftStick & Keyboard/2
R3 = SDL-0/RightStick & Keyboard/4
LUp = SDL-0/-LeftY & Keyboard/W
LRight = SDL-0/+LeftX & Keyboard/D
LDown = SDL-0/+LeftY & Keyboard/S
LLeft = SDL-0/-LeftX & Keyboard/A
RUp = SDL-0/-RightY & Keyboard/T
RRight = SDL-0/+RightX & Keyboard/H
RDown = SDL-0/+RightY & Keyboard/G
RLeft = SDL-0/-RightX & Keyboard/F
SmallMotor = SDL-0/SmallMotor
LargeMotor = SDL-0/LargeMotor
"#;

/// Body of `[Pad1]` we write — minus the section header so we can also
/// hot-patch into an existing PCSX2.ini.
const PCSX2_PAD1_BODY: &str = "\
Type = DualShock2
InvertL = 0
InvertR = 0
Deadzone = 0
AxisScale = 1.33
LargeMotorScale = 1
SmallMotorScale = 1
ButtonDeadzone = 0
PressureModifier = 0.5
Up = SDL-0/DPadUp & Keyboard/Up
Right = SDL-0/DPadRight & Keyboard/Right
Down = SDL-0/DPadDown & Keyboard/Down
Left = SDL-0/DPadLeft & Keyboard/Left
Triangle = SDL-0/Y & Keyboard/I
Circle = SDL-0/B & Keyboard/L
Cross = SDL-0/A & Keyboard/K
Square = SDL-0/X & Keyboard/J
Select = SDL-0/Back & Keyboard/Backspace
Start = SDL-0/Start & Keyboard/Return
L1 = SDL-0/LeftShoulder & Keyboard/Q
L2 = SDL-0/+LeftTrigger & Keyboard/1
R1 = SDL-0/RightShoulder & Keyboard/E
R2 = SDL-0/+RightTrigger & Keyboard/3
L3 = SDL-0/LeftStick & Keyboard/2
R3 = SDL-0/RightStick & Keyboard/4
LUp = SDL-0/-LeftY & Keyboard/W
LRight = SDL-0/+LeftX & Keyboard/D
LDown = SDL-0/+LeftY & Keyboard/S
LLeft = SDL-0/-LeftX & Keyboard/A
RUp = SDL-0/-RightY & Keyboard/T
RRight = SDL-0/+RightX & Keyboard/H
RDown = SDL-0/+RightY & Keyboard/G
RLeft = SDL-0/-RightX & Keyboard/F
SmallMotor = SDL-0/SmallMotor
LargeMotor = SDL-0/LargeMotor";

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
///   1. PCSX2.ini doesn't exist → write the full seed.
///   2. PCSX2.ini exists with the keyboard-only defaults → patch
///      `[InputSources] XInput = true` and replace `[Pad1]` in place.
///   3. PCSX2.ini exists with user-customised Pad1 → leave alone.
///
/// Returns `Ok(true)` if we modified anything, `Ok(false)` otherwise.
pub fn ensure_pcsx2_default() -> Result<bool, String> {
    let Some(ini) = pcsx2_ini_path() else { return Ok(false) };

    // Case 1: file doesn't exist → seed.
    if !ini.exists() {
        if let Some(parent) = ini.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("creating {}: {e}", parent.display()))?;
        }
        std::fs::write(&ini, PCSX2_INI_SEED)
            .map_err(|e| format!("writing {}: {e}", ini.display()))?;
        log::info!("controller_setup: seeded fresh PCSX2.ini with gamepad bindings");
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

    let text = ensure_input_source(&text, "XInput", "true");
    let text = ensure_input_source(&text, "SDL", "true");
    let text = replace_section(&text, "[Pad1]", &format!("[Pad1]\n{PCSX2_PAD1_BODY}"));
    std::fs::write(&ini, text)
        .map_err(|e| format!("writing {}: {e}", ini.display()))?;
    log::info!("controller_setup: patched default PCSX2.ini → Pad1 bound to SDL-0 (XInput)");
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
        // Insert immediately after the section header.
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

/// Default DuckStation `settings.ini` snippet: pad-1 is an Analog
/// Controller bound to the first SDL gamepad with keyboard as fallback.
/// Source = SDL means DuckStation listens for hot-plug events and re-
/// binds slot 0 to whatever pad gets plugged in next.
const DUCKSTATION_PAD1_BLOCK: &str = "\
[Pad1]
Type = AnalogController
Up = SDL-0/DPadUp & Keyboard/W
Down = SDL-0/DPadDown & Keyboard/S
Left = SDL-0/DPadLeft & Keyboard/A
Right = SDL-0/DPadRight & Keyboard/D
Triangle = SDL-0/Y & Keyboard/I
Circle = SDL-0/B & Keyboard/L
Cross = SDL-0/A & Keyboard/K
Square = SDL-0/X & Keyboard/J
Select = SDL-0/Back & Keyboard/Backspace
Start = SDL-0/Start & Keyboard/Return
L1 = SDL-0/LeftShoulder & Keyboard/Q
L2 = SDL-0/+LeftTrigger & Keyboard/1
R1 = SDL-0/RightShoulder & Keyboard/E
R2 = SDL-0/+RightTrigger & Keyboard/3
L3 = SDL-0/LeftStick & Keyboard/2
R3 = SDL-0/RightStick & Keyboard/4
LLeftRight = SDL-0/LeftX
LUpDown = SDL-0/LeftY
RLeftRight = SDL-0/RightX
RUpDown = SDL-0/RightY
SmallMotor = SDL-0/SmallMotor
LargeMotor = SDL-0/LargeMotor";

#[cfg(target_os = "windows")]
fn duckstation_ini_path() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(local).join("DuckStation").join("settings.ini"))
}

#[cfg(not(target_os = "windows"))]
fn duckstation_ini_path() -> Option<PathBuf> { None }

/// Seed DuckStation's Pad1 binding to the first SDL gamepad. Idempotent:
/// only writes the [Pad1] block when settings.ini doesn't have one yet
/// or has only the default empty/keyboard binding.
pub fn ensure_duckstation_default() -> Result<bool, String> {
    let Some(ini) = duckstation_ini_path() else { return Ok(false) };
    if !ini.exists() {
        if let Some(parent) = ini.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("creating {}: {e}", parent.display()))?;
        }
        let seed = format!("\
[InputSources]
SDL = true
XInput = false
DInput = false

{DUCKSTATION_PAD1_BLOCK}
");
        std::fs::write(&ini, seed)
            .map_err(|e| format!("writing {}: {e}", ini.display()))?;
        log::info!("controller_setup: seeded fresh DuckStation settings.ini");
        return Ok(true);
    }

    let text = std::fs::read_to_string(&ini)
        .map_err(|e| format!("reading {}: {e}", ini.display()))?;
    // Already has a non-trivial Pad1? Leave alone.
    if text.contains("[Pad1]") && text.contains("SDL-0/") {
        return Ok(false);
    }
    let text = ensure_input_source(&text, "SDL", "true");
    let text = replace_section(&text, "[Pad1]", DUCKSTATION_PAD1_BLOCK);
    std::fs::write(&ini, text)
        .map_err(|e| format!("writing {}: {e}", ini.display()))?;
    log::info!("controller_setup: patched DuckStation settings.ini Pad1 → SDL-0");
    Ok(true)
}

/// Default Dolphin GCPad1 binding to XInput controller 0. Dolphin's
/// SDL/XInput backend auto-detects pads on hot-plug, but a missing
/// GCPadNew.ini means GCPad1 stays unbound until the user opens
/// Options → Controllers. Writing this file when none exists removes
/// that extra click.
const DOLPHIN_GCPAD1_BLOCK: &str = "\
[GCPad1]
Device = XInput/0/Gamepad
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
Rumble/Motor = `Motor L`|`Motor R`";

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

/// Seed Dolphin's GCPadNew.ini with a working XInput binding when none
/// exists. Conservative: if the file is already there at all (even just
/// the empty stub Dolphin writes on its own first launch) we leave it
/// alone — Dolphin's UI rewrites this file frequently and clobbering a
/// user's config would be much worse than the missing initial binding.
pub fn ensure_dolphin_default() -> Result<bool, String> {
    let Some(ini) = dolphin_gcpad_ini_path() else { return Ok(false) };
    if ini.exists() { return Ok(false); }
    if let Some(parent) = ini.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("creating {}: {e}", parent.display()))?;
    }
    std::fs::write(&ini, DOLPHIN_GCPAD1_BLOCK)
        .map_err(|e| format!("writing {}: {e}", ini.display()))?;
    log::info!("controller_setup: seeded fresh Dolphin GCPadNew.ini for XInput pad 0");
    Ok(true)
}

/// Apply every per-emulator default we know about. PCSX2 and DuckStation
/// are the two PlayStation emulators whose first-run config binds Pad 1
/// to keyboard only. Dolphin gets a seed only when GCPadNew.ini is
/// missing entirely — its UI rewrites the file constantly so we never
/// patch it in place. Everything else (PPSSPP, Snes9x, mGBA, DeSmuME,
/// Stella, Simple64, Flycast, RPCS3) uses SDL2 directly and auto-detects
/// gamepads on hot-plug without intervention.
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
    fn dolphin_seed_block_binds_to_xinput_pad_0() {
        // Sanity-check the embedded string the writer would drop on disk.
        // If anyone reformats the constant we want CI to fail loud.
        assert!(DOLPHIN_GCPAD1_BLOCK.contains("[GCPad1]"));
        assert!(DOLPHIN_GCPAD1_BLOCK.contains("Device = XInput/0/Gamepad"));
        // Spot-check a few essential bindings — GC games rely on all
        // four face buttons + the analog triggers being wired.
        for needle in ["Buttons/A", "Buttons/Start", "Triggers/L", "Triggers/R",
                       "Main Stick/Up", "C-Stick/Up", "D-Pad/Up", "Rumble/Motor"] {
            assert!(DOLPHIN_GCPAD1_BLOCK.contains(needle),
                "Dolphin seed missing binding {needle}");
        }
    }
}
