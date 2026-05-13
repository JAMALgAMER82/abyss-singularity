use serde::{Deserialize, Serialize};

/// What family of gamepad we're looking at. Maps to a different button
/// layout convention in the autoconfig generator below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerKind {
    Xbox,
    PlayStation,
    SwitchPro,
    /// Anything we don't recognise — fall back to a sane Xbox-ish default.
    Generic,
}

impl ControllerKind {
    /// "Smart" detection from the `Gamepad.id` string the Web Gamepad
    /// API hands the frontend (e.g. "Xbox 360 Controller (XInput STANDARD GAMEPAD)").
    /// We can also hit this from the Rust side via a Tauri command — both
    /// sides agree on the heuristic so user expectations stay aligned.
    pub fn detect_from_id(id: &str) -> Self {
        let lower = id.to_lowercase();
        if lower.contains("xbox") || lower.contains("xinput") || lower.contains("microsoft") {
            ControllerKind::Xbox
        } else if lower.contains("dualshock") || lower.contains("dualsense")
            || lower.contains("playstation") || lower.contains("sony")
            || lower.contains("ds4") || lower.contains("ds5")
            || lower.contains("wireless controller")
        {
            ControllerKind::PlayStation
        } else if lower.contains("nintendo") || lower.contains("pro controller")
            || lower.contains("switch") || lower.contains("joy-con")
        {
            ControllerKind::SwitchPro
        } else {
            ControllerKind::Generic
        }
    }

    /// Suggested driver name in RetroArch's autoconfig — XInput-style
    /// pads get `xinput`, everything else `dinput` on Windows.
    pub fn retroarch_driver(&self) -> &'static str {
        match self {
            ControllerKind::Xbox => "xinput",
            _ => "dinput",
        }
    }
}

/// Result of `controller_apply_to_retroarch`. Useful for the UI's
/// success toast.
#[derive(Debug, Clone, Serialize)]
pub struct AutoConfigReport {
    pub written_to: std::path::PathBuf,
    pub bytes:      usize,
}
