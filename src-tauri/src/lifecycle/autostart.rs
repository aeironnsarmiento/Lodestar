//! Start at login (R8, KTD14): an HKCU Run entry on Windows, a LaunchAgent on macOS,
//! launching `--minimized`, so the app starts in the tray and brings up servers
//! flagged to start with it.

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

pub const MINIMIZED_ARG: &str = "--minimized";

pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![MINIMIZED_ARG]))
}

/// Adds or removes the login entry to match the setting.
pub fn apply(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    let current = manager.is_enabled().unwrap_or(false);
    if enabled == current {
        return Ok(());
    }
    let result = if enabled { manager.enable() } else { manager.disable() };
    result.map_err(|e| format!("Could not change start at login: {e}"))
}

/// Removes the Run entry left by the app's old name (Glasscraft), so the old build
/// never starts alongside this one. Missing entries are fine. Glasscraft was
/// Windows-only, so there is nothing to remove elsewhere.
#[cfg(not(windows))]
pub fn remove_legacy_entry() {}

#[cfg(windows)]
pub fn remove_legacy_entry() {
    use windows_sys::Win32::System::Registry::{RegDeleteKeyValueW, HKEY_CURRENT_USER};
    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let value = wide("Glasscraft");
    // SAFETY: both strings are NUL-terminated UTF-16 buffers that outlive the call.
    unsafe {
        RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr());
    }
}

pub fn launched_minimized() -> bool {
    std::env::args().any(|a| a == MINIMIZED_ARG)
}
