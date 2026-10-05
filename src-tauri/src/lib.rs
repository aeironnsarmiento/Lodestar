pub mod commands;
pub mod core;
pub mod download;
pub mod java;
pub mod providers;

use std::sync::Arc;

use tauri::Manager;

use crate::core::app::{App, AppConfig};
use crate::core::events::TauriSink;
use crate::core::paths::Paths;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|tauri_app| {
            let events = Arc::new(TauriSink(tauri_app.handle().clone()));
            let app = App::new(AppConfig::new(Paths::new(Paths::default_root())), events)?;
            tauri_app.manage(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::instances::list_instances,
            commands::instances::create_instance,
            commands::instances::update_instance,
            commands::instances::delete_instance,
            commands::instances::list_versions,
            commands::java::list_java_runtimes,
            commands::java::remove_java_runtime,
            commands::settings::get_settings,
            commands::settings::set_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
