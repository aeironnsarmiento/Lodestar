use super::{err, AppState, CmdResult};
use crate::core::settings::AppSettings;
use crate::lifecycle::{autostart, tray};

#[tauri::command]
pub fn get_settings(app: AppState<'_>) -> AppSettings {
    app.settings()
}

/// Saves settings; turning start-with-Windows on or off updates the Run entry first,
/// so a failure there leaves the saved setting unchanged.
#[tauri::command]
pub fn set_settings(app: AppState<'_>, handle: tauri::AppHandle, settings: AppSettings) -> CmdResult<AppSettings> {
    if settings.start_with_windows != app.settings().start_with_windows {
        autostart::apply(&handle, settings.start_with_windows)?;
    }
    app.set_settings(settings).map_err(err)
}

#[tauri::command]
pub fn accept_eula(app: AppState<'_>) -> CmdResult<AppSettings> {
    app.accept_eula().map_err(err)
}

/// Quit from the UI: stops every server, then exits.
#[tauri::command]
pub fn quit_app(handle: tauri::AppHandle) {
    tray::quit(&handle);
}
