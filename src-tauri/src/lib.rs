pub mod addons;
pub mod commands;
pub mod core;
pub mod download;
pub mod java;
pub mod lifecycle;
pub mod playit;
pub mod providers;
pub mod supervisor;
pub mod worlds;

use std::sync::Arc;

use tauri::{Manager, RunEvent, WindowEvent};

use crate::core::app::{App, AppConfig};
use crate::core::events::TauriSink;
use crate::core::paths::Paths;
use crate::lifecycle::{autostart, tray};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch focuses the running app instead.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_main(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(autostart::plugin())
        .setup(|tauri_app| {
            let handle = tauri_app.handle().clone();
            let events = Arc::new(TauriSink(handle.clone()));
            let root = Paths::default_root();
            if let Err(e) = Paths::migrate_legacy_root(&root) {
                eprintln!("lodestar: could not move the old Glasscraft data folder: {e}");
            }
            autostart::remove_legacy_entry();
            let app = App::new(AppConfig::new(Paths::new(root)), events)?;
            tauri_app.manage(app.clone());

            #[cfg(target_os = "macos")]
            handle.set_menu(tray::app_menu(&handle)?)?;
            tray::install(&handle)?;
            if autostart::launched_minimized() {
                if let Some(w) = handle.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            // Keep the Run entry in step with the setting (e.g. after the app moved).
            let _ = autostart::apply(&handle, app.settings().start_with_windows);

            tauri::async_runtime::spawn(async move { app.start_background() });
            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id() == tray::APP_QUIT {
                tray::quit(app);
            }
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() != "main" {
                    return;
                }
                api.prevent_close();
                let app = window.app_handle();
                let close_to_tray = app.state::<Arc<App>>().settings().close_to_tray;
                if close_to_tray {
                    let _ = window.hide();
                } else {
                    tray::quit(app);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::instances::list_instances,
            commands::instances::create_instance,
            commands::instances::update_instance,
            commands::instances::delete_instance,
            commands::instances::list_versions,
            commands::instances::retry_provision,
            commands::instances::open_addons_folder,
            commands::java::list_java_runtimes,
            commands::java::remove_java_runtime,
            commands::servers::start_server,
            commands::servers::stop_server,
            commands::servers::restart_server,
            commands::servers::kill_server,
            commands::servers::send_command,
            commands::servers::get_console,
            commands::servers::server_snapshots,
            commands::servers::join_info,
            commands::worlds::list_worlds,
            commands::worlds::reset_world,
            commands::worlds::switch_world,
            commands::playit::playit_status,
            commands::playit::playit_setup,
            commands::playit::playit_cancel,
            commands::playit::playit_relink,
            commands::playit::playit_retry_tunnel,
            commands::settings::get_settings,
            commands::settings::set_settings,
            commands::settings::accept_eula,
            commands::settings::quit_app,
            commands::addons::list_addons,
            commands::addons::identify_addons,
            commands::addons::set_addon_enabled,
            commands::addons::remove_addon,
            commands::addons::import_addons,
            commands::addons::search_projects,
            commands::addons::project_versions,
            commands::addons::install_project,
            commands::addons::check_addon_updates,
            commands::addons::update_addon,
            commands::addons::inspect_modpack_file,
            commands::addons::update_modpack,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            // Quit from the Dock or at logout: stop servers gracefully first. The
            // final `exit(0)` from `tray::quit` carries a code and goes through.
            RunEvent::ExitRequested { code: None, api, .. } => {
                api.prevent_exit();
                tray::quit(app);
            }
            // Clicking the Dock icon brings back a window hidden to the menu bar.
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { has_visible_windows: false, .. } => tray::show_main(app),
            _ => {}
        });
}
