// Abyss Singularity — Tauri backend entry point.
//
// Phase 1: bring up the window with a logger attached.
// Phase 2: library scanner + cache + IGDB enrichment.
// Phase 3: emulator orchestration (this turn).
// Subsequent phases register their commands & plugins here:
//   - Phase 4 (Networking):  Tailscale orchestration + latency probes
//   - Phase 5 (Streaming):   Sunshine/Moonlight process supervisor
//   - Phase 6 (Social):      WebRTC signalling

mod chat;
mod controller;
mod installer;
mod library;
mod mesh;
mod network;
mod orchestration;
mod streaming;
mod transfer;

use std::sync::Arc;

use orchestration::launcher::ProcessRegistry;
use streaming::commands::HostState;
use tauri_plugin_log::{Target, TargetKind};

/// Health-check command exposed to the frontend. Useful during bring-up and
/// as a smoke test that the IPC bridge is functioning end-to-end.
#[tauri::command]
fn abyss_ping() -> &'static str {
    "abyss-online"
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Log to stdout *and* a rotating file under the app's log dir, so
        // we have a transcript when orchestrating external binaries in
        // Phases 3-5 — per the project's explicit logging requirement.
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                    Target::new(TargetKind::Webview),
                ])
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // Second-launch handler: bring the existing window forward
            // instead of spawning a duplicate process tree (and another
            // mesh sidecar fighting for the same ports).
            use tauri::Manager as _;
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Spawn the embedded Tailscale mesh sidecar at app start.
            // If this fails (e.g. binary missing in a dev build), we log
            // and continue — the user still gets the rest of the UI.
            let handle = app.handle().clone();
            if let Err(e) = mesh::sidecar::spawn(&handle) {
                log::warn!("mesh: sidecar spawn failed: {e:#}");
            }
            // Bring the file-transfer listener up so peers can send us
            // games at any time. Cheap — just one localhost socket.
            let xfer_app = handle.clone();
            tauri::async_runtime::spawn(async move {
                let state = transfer::state::global();
                if let Err(e) = transfer::server::run(xfer_app, state).await {
                    log::error!("transfer: listener task ended: {e:#}");
                }
            });

            // System tray: keeps the mesh + chat alive in the background
            // when the user closes the window. Right-click menu lets them
            // restore the UI or quit fully.
            install_system_tray(app)?;

            Ok(())
        })
        .manage(Arc::new(ProcessRegistry::default()))
        .manage(Arc::new(HostState::default()))
        .invoke_handler(tauri::generate_handler![
            abyss_ping,
            library::commands::library_get_config,
            library::commands::library_set_config,
            library::commands::library_add_path,
            library::commands::library_remove_path,
            library::commands::library_load,
            library::commands::library_scan,
            library::commands::library_set_igdb_credentials,
            library::commands::library_enrich_metadata,
            orchestration::commands::orch_get_config,
            orchestration::commands::orch_set_config,
            orchestration::commands::orch_builtin_recipes,
            orchestration::commands::orch_launch,
            orchestration::commands::orch_terminate,
            orchestration::commands::orch_list_running,
            network::commands::net_tailscale_status,
            network::commands::net_probe_regions,
            network::commands::net_my_profile,
            network::commands::net_recommend_pair,
            streaming::commands::stream_get_config,
            streaming::commands::stream_set_config,
            streaming::commands::stream_add_host,
            streaming::commands::stream_remove_host,
            streaming::commands::stream_host_status,
            streaming::commands::stream_start_host,
            streaming::commands::stream_stop_host,
            streaming::commands::stream_launch_client,
            chat::commands::chat_get_config,
            chat::commands::chat_set_config,
            chat::commands::chat_start,
            chat::commands::chat_stop,
            chat::commands::chat_status,
            chat::commands::chat_connect_peer,
            chat::commands::chat_send,
            chat::commands::chat_get_history,
            chat::commands::chat_get_peers,
            chat::commands::chat_set_presence,
            installer::commands::installer_available,
            installer::commands::installer_status,
            installer::commands::installer_install,
            installer::commands::installer_uninstall,
            installer::commands::installer_auto_assign,
            transfer::commands::transfer_start,
            transfer::commands::transfer_send,
            transfer::commands::transfer_accept,
            transfer::commands::transfer_reject,
            transfer::commands::transfer_list_incoming,
            controller::commands::controller_detect_kind,
            controller::commands::controller_apply_to_retroarch,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn install_system_tray<R: tauri::Runtime>(app: &tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let show = MenuItem::with_id(app, "tray:show", "Show Abyss",       true, None::<&str>)?;
    let sep  = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "tray:quit", "Quit Abyss",       true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &sep, &quit])?;

    let _tray = TrayIconBuilder::with_id("abyss-tray")
        .tooltip("Abyss Singularity")
        .icon(app.default_window_icon().expect("default icon").clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            match event.id.as_ref() {
                "tray:show" => focus_main_window(app),
                "tray:quit" => app.exit(0),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            // Left-click on the tray icon brings the window forward —
            // standard Windows app convention.
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                focus_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn focus_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    use tauri::Manager as _;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}
