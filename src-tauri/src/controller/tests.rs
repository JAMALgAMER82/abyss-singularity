use super::retroarch::{build_config, safe_filename};
use super::types::ControllerKind;

#[test]
fn detect_xbox_kind_from_xinput_id() {
    assert_eq!(
        ControllerKind::detect_from_id("Xbox 360 Controller (XInput STANDARD GAMEPAD)"),
        ControllerKind::Xbox,
    );
    assert_eq!(
        ControllerKind::detect_from_id("Microsoft X-Box One pad"),
        ControllerKind::Xbox,
    );
}

#[test]
fn detect_playstation_kind_from_ds_ids() {
    for id in [
        "Sony Computer Entertainment Wireless Controller",
        "DualShock 4 Wireless Controller",
        "DualSense Wireless Controller",
        "Wireless Controller", // PS4 over Bluetooth
    ] {
        assert_eq!(ControllerKind::detect_from_id(id), ControllerKind::PlayStation, "id was: {id}");
    }
}

#[test]
fn detect_switch_pro_from_ids() {
    for id in ["Nintendo Switch Pro Controller", "Pro Controller", "Nintendo Co., Ltd. Joy-Con (L)"] {
        assert_eq!(ControllerKind::detect_from_id(id), ControllerKind::SwitchPro, "id was: {id}");
    }
}

#[test]
fn detect_falls_back_to_generic() {
    assert_eq!(ControllerKind::detect_from_id("8BitDo SN30 Pro"), ControllerKind::Generic);
}

#[test]
fn retroarch_driver_picks_xinput_for_xbox() {
    assert_eq!(ControllerKind::Xbox.retroarch_driver(), "xinput");
    assert_eq!(ControllerKind::PlayStation.retroarch_driver(), "dinput");
    assert_eq!(ControllerKind::SwitchPro.retroarch_driver(), "dinput");
    assert_eq!(ControllerKind::Generic.retroarch_driver(), "dinput");
}

#[test]
fn build_config_emits_all_required_keys() {
    let cfg = build_config(ControllerKind::Xbox, "Xbox 360 Controller");
    for key in [
        "input_device = \"Xbox 360 Controller\"",
        "input_driver = \"xinput\"",
        "input_b_btn",
        "input_a_btn",
        "input_x_btn",
        "input_y_btn",
        "input_l_btn",
        "input_r_btn",
        "input_select_btn",
        "input_start_btn",
        "input_up_btn",
        "input_down_btn",
        "input_left_btn",
        "input_right_btn",
        "input_l_x_minus_axis",
        "input_l_y_plus_axis",
    ] {
        assert!(cfg.contains(key), "config missing key {key}:\n{cfg}");
    }
}

#[test]
fn safe_filename_strips_windows_reserved_chars() {
    assert_eq!(safe_filename("Foo/Bar:Baz"),     "Foo_Bar_Baz");
    assert_eq!(safe_filename("ok name"),         "ok name");
    assert_eq!(safe_filename(r#"<weird*name?>"#), "_weird_name__");
}
