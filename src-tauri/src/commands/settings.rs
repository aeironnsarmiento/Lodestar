use super::{err, AppState, CmdResult};
use crate::core::settings::AppSettings;

#[tauri::command]
pub fn get_settings(app: AppState<'_>) -> AppSettings {
    app.settings()
}

#[tauri::command]
pub fn set_settings(app: AppState<'_>, settings: AppSettings) -> CmdResult<AppSettings> {
    app.set_settings(settings).map_err(err)
}

#[tauri::command]
pub fn accept_eula(app: AppState<'_>) -> CmdResult<AppSettings> {
    app.accept_eula().map_err(err)
}
