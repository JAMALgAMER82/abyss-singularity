//! RetroArch joypad-autoconfig generator.
//!
//! Format reference: https://docs.libretro.com/library/retroarch/input/autoconfig/
//!
//! When RetroArch sees a gamepad whose ID matches a `.cfg` file in its
//! `autoconfig/<driver>/` directory, it loads that file's button bindings
//! for the pad. By writing a file there we can pre-configure the user's
//! controller without them ever opening RetroArch's settings.

use super::types::ControllerKind;

/// Build the `.cfg` body for a given controller kind + name.
///
/// `controller_name` becomes the `input_device` value — RetroArch matches
/// this against the connected gamepad's name. Pass exactly what the
/// frontend's Web Gamepad API reported.
pub fn build_config(kind: ControllerKind, controller_name: &str) -> String {
    let driver = kind.retroarch_driver();
    // Canonical Xbox-style mapping. RetroArch's "B" is the bottom face
    // button (south), "A" is east — matches the Sony / Nintendo
    // convention of "confirm = bottom". Standard Web Gamepad API maps:
    //   0 = south (A on Xbox, X on PS, B on Switch)
    //   1 = east  (B on Xbox, O on PS, A on Switch)
    //   2 = west  (X on Xbox, □ on PS, Y on Switch)
    //   3 = north (Y on Xbox, △ on PS, X on Switch)
    //   4/5 = L1/R1   6/7 = L2/R2 (triggers — we use as axes too)
    //   8 = select   9 = start  10/11 = L3/R3
    //   12-15 = dpad up/down/left/right
    let (b_btn, a_btn, y_btn, x_btn,
         l_btn, r_btn,
         l2_btn, r2_btn,
         select_btn, start_btn,
         l3_btn, r3_btn,
         up_btn, down_btn, left_btn, right_btn)
    = match kind {
        // For all four kinds the *physical button indexes* are the same
        // (Web Gamepad API normalises to "standard mapping"). What
        // differs is the user expectation of which physical button is
        // "confirm" — we leave RetroArch's logical b/a in the canonical
        // south/east positions because RetroArch handles the per-system
        // remap itself.
        ControllerKind::Xbox
        | ControllerKind::PlayStation
        | ControllerKind::SwitchPro
        | ControllerKind::Generic => (
            0,  1,  3,  2,
            4,  5,
            6,  7,
            8,  9,
            10, 11,
            12, 13, 14, 15,
        ),
    };

    format!(
"input_device = \"{controller_name}\"
input_driver = \"{driver}\"
input_b_btn = \"{b_btn}\"
input_a_btn = \"{a_btn}\"
input_y_btn = \"{y_btn}\"
input_x_btn = \"{x_btn}\"
input_l_btn = \"{l_btn}\"
input_r_btn = \"{r_btn}\"
input_l2_btn = \"{l2_btn}\"
input_r2_btn = \"{r2_btn}\"
input_select_btn = \"{select_btn}\"
input_start_btn = \"{start_btn}\"
input_l3_btn = \"{l3_btn}\"
input_r3_btn = \"{r3_btn}\"
input_up_btn = \"{up_btn}\"
input_down_btn = \"{down_btn}\"
input_left_btn = \"{left_btn}\"
input_right_btn = \"{right_btn}\"
input_l_x_minus_axis = \"-0\"
input_l_x_plus_axis = \"+0\"
input_l_y_minus_axis = \"-1\"
input_l_y_plus_axis = \"+1\"
input_r_x_minus_axis = \"-2\"
input_r_x_plus_axis = \"+2\"
input_r_y_minus_axis = \"-3\"
input_r_y_plus_axis = \"+3\"
"
    )
}

/// Sanitise a controller name into a filename RetroArch will accept.
/// RetroArch is permissive but Windows is not — strip path separators
/// and reserved chars.
pub fn safe_filename(controller_name: &str) -> String {
    controller_name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}
