// Abyss Singularity — Tauri backend entry point.
//
// Phase 1: bring up the window with a logger attached.
// Phase 2: library scanner + cache + IGDB enrichment.
// Phase 3: emulator orchestration (this turn).
// Subsequent phases register their commands & plugins here:
//   - Phase 4 (Networking):  Tailscale orchestration + latency probes
//   - Phase 5 (Streaming):   Sunshine/Moonlight process supervisor
//   - Phase 6 (Social):      P2P chat over the embedded mesh
//                            (raw length-prefixed JSON frames, not WebRTC)

mod chat;
mod controller;
mod diagnostics;
mod directory;
mod installer;
mod library;
mod lobby;
mod mesh;
mod network;
mod orchestration;
mod streaming;
mod transfer;
mod util;

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
        // X-button → hide to tray instead of exiting. The mesh sidecar,
        // chat listener, transfer server, and directory heartbeat keep
        // running in the background so friends can still reach the user.
        // "Quit Abyss" in the tray menu (or app.exit() anywhere else) is
        // the only path that actually terminates the process tree.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
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

            // Ensure SunshineService is set to auto-start AND currently
            // running. Cheap (a couple of `sc` calls) and idempotent —
            // no-ops if Sunshine isn't installed yet (auto-installer
            // below will set it up) or if everything is already in the
            // right state. Runs on a thread so a slow `sc query` can't
            // block the setup callback.
            std::thread::spawn(|| {
                match installer::streaming_apps::ensure_sunshine_running() {
                    Ok(true)  => log::info!("streaming: ensured Sunshine running on startup"),
                    Ok(false) => {}
                    Err(e)    => log::warn!("streaming: ensure_sunshine_running failed: {e:#}"),
                }
            });

            // Self-repair: re-scan emulator install folders and reconcile
            // the orchestration config. Picks up emulators that extracted
            // into a versioned subdir (Cemu) or that previously failed and
            // have since been completed manually. Cheap (idempotent walk).
            let repair_app = handle.clone();
            tauri::async_runtime::spawn(async move {
                match installer::commands::installer_repair(repair_app) {
                    Ok(n) if n > 0 => log::info!("installer: repaired {n} emulator config entries on startup"),
                    Ok(_)          => {}
                    Err(e)         => log::warn!("installer: startup repair failed: {e}"),
                }
            });

            // Auto-initialise the directory config so a fresh install lands
            // on the Discover tab with a working identity (UUID + handle +
            // baked-in Worker URL) and starts heartbeating without the user
            // having to touch Settings.
            if let Err(e) = directory::config::ensure_initialized(&handle) {
                log::warn!("directory: auto-init failed: {e:#}");
            }
            // Directory client — heartbeat + inbox poll. No-ops silently
            // when the user hasn't pasted a Worker URL yet, so it's safe
            // to always spawn.
            directory::heartbeat::spawn(handle.clone());

            // First-launch auto-install of every supported emulator (~600 MB
            // total). Same skip-if-attempted pattern as streaming apps below.
            // `installer_install_all` marks the LibraryConfig timestamp at
            // its very start so a wizard-button click and this background
            // task can't both download the same emulator concurrently —
            // whichever fires first wins the timestamp, the loser bails.
            // Sleeps 30 s after launch so the window is rendered, the
            // streaming-apps installer's UAC dialogs (if any) have come
            // and gone, and the user isn't hit by multiple modals at once.
            let emu_app = handle.clone();
            tauri::async_runtime::spawn(async move {
                match library::config::load(&emu_app) {
                    Ok(cfg) if cfg.emulators_install_attempted_at.is_some() => return,
                    Ok(_)  => {}
                    Err(_) => return,
                }
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                // Double-check: user may have clicked the wizard's "Install
                // all" button during the 30 s wait; re-loading config tells
                // us. The command itself also re-checks but this saves us
                // the spawn round-trip.
                if let Ok(cfg) = library::config::load(&emu_app) {
                    if cfg.emulators_install_attempted_at.is_some() { return; }
                }
                log::info!("first-launch: auto-installing all 13 emulators (~600 MB background download)");
                let app_for_cmd = emu_app.clone();
                match installer::commands::installer_install_all(app_for_cmd).await {
                    Ok(report) => log::info!(
                        "first-launch emulator install: {} installed, {} already present, {} failed",
                        report.installed.len(),
                        report.already_present.len(),
                        report.failed.len(),
                    ),
                    Err(e) => log::warn!("first-launch emulator install: {e}"),
                }
            });

            // First-launch auto-install of Sunshine + Moonlight + Tailscale.
            // The underlying `install_both` is idempotent: each app's
            // detect_existing check skips if already installed. We mark a
            // timestamp in LibraryConfig after the attempt completes (success
            // or fail) so we don't pop UAC prompts on every subsequent
            // launch. Power users who declined can manually re-trigger from
            // Settings → Streaming → Install streaming apps.
            let stream_app = handle.clone();
            tauri::async_runtime::spawn(async move {
                // Bail if we've already tried — don't re-bug the user.
                match library::config::load(&stream_app) {
                    Ok(cfg) if cfg.streaming_apps_attempted_at.is_some() => return,
                    Ok(_)  => {}
                    Err(_) => return,
                }
                // Bail if everything's already installed — skip the
                // attempted-at write too so power users with pre-existing
                // installs aren't marked "done" and we'd still auto-install
                // if e.g. they uninstall Moonlight later.
                let pre = installer::streaming_apps::detect_existing();
                if pre.sunshine_installed && pre.moonlight_installed && pre.tailscale_installed {
                    return;
                }
                // Wait a bit so the main window has time to render before
                // the UAC prompt potentially steals focus. Friend-side
                // UX: they double-click the installer, app opens, UAC for
                // streaming apps follows within ~15 s.
                tokio::time::sleep(std::time::Duration::from_secs(15)).await;
                log::info!("first-launch: auto-installing streaming apps (Sunshine + Moonlight + Tailscale)");
                let app_for_cmd = stream_app.clone();
                match installer::commands::installer_install_streaming_apps(app_for_cmd).await {
                    Ok(_)  => log::info!("first-launch streaming install: complete"),
                    Err(e) => log::warn!("first-launch streaming install: {e}"),
                }
                // Mark attempted regardless of outcome.
                if let Ok(mut cfg) = library::config::load(&stream_app) {
                    cfg.streaming_apps_attempted_at = Some(chrono::Utc::now());
                    if let Err(e) = library::config::save(&stream_app, &cfg) {
                        log::warn!("first-launch: marking streaming-apps attempted failed: {e}");
                    }
                }
            });

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
            network::commands::net_get_config,
            network::commands::net_set_invite_config,
            network::commands::net_create_invite,
            network::commands::net_redeem_invite,
            network::commands::net_clear_redeemed_invite,
            lobby::commands::lobby_state,
            lobby::commands::lobby_host_room,
            lobby::commands::lobby_close_room,
            lobby::commands::lobby_request_join,
            lobby::commands::lobby_leave_room,
            lobby::commands::lobby_start_game,
            directory::commands::dir_get_config,
            directory::commands::dir_set_config,
            directory::commands::dir_online,
            directory::commands::dir_send_friend_request,
            directory::commands::dir_friend_requests,
            directory::commands::dir_friend_responses,
            directory::commands::dir_accept_request,
            directory::commands::dir_reject_request,
            directory::commands::dir_friends,
            directory::commands::dir_send_dm,
            directory::commands::dir_get_dms,
            directory::commands::dir_send_global_chat,
            directory::commands::dir_get_global_chat,
            directory::commands::dir_block,
            directory::commands::dir_unblock,
            streaming::commands::stream_get_config,
            streaming::commands::stream_set_config,
            streaming::commands::stream_add_host,
            streaming::commands::stream_remove_host,
            streaming::commands::stream_host_status,
            streaming::commands::stream_start_host,
            streaming::commands::stream_stop_host,
            streaming::commands::stream_launch_client,
            streaming::commands::stream_pair_client,
            streaming::commands::stream_reset_credentials,
            streaming::pairing::stream_request_pair_and_launch,
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
            installer::commands::installer_repair,
            installer::commands::installer_install_all,
            installer::commands::installer_configure_controllers,
            installer::commands::installer_install_rpcs3_firmware,
            installer::commands::installer_autodetect_bios,
            installer::commands::installer_install_bios_file,
            installer::commands::installer_install_streaming_apps,
            diagnostics::commands::diagnostics_run_all,
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
