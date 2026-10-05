//! Start with Windows (R8, KTD14): an HKCU Run entry launching `--minimized`, so the
//! app starts in the tray and brings up servers flagged to start with it.

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

pub const MINIMIZED_ARG: &str = "--minimized";

pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![MINIMIZED_ARG]))
}

/// Adds or removes the Run entry to match the setting.
pub fn apply(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    let current = manager.is_enabled().unwrap_or(false);
    if enabled == current {
        return Ok(());
    }
    let result = if enabled { manager.enable() } else { manager.disable() };
    result.map_err(|e| format!("Could not change start-with-Windows: {e}"))
}

pub fn launched_minimized() -> bool {
    std::env::args().any(|a| a == MINIMIZED_ARG)
}
