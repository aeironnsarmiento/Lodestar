use tauri_plugin_opener::OpenerExt;

use super::{err, AppState, CmdResult};
use crate::playit::PlayitStatus;

#[tauri::command]
pub fn playit_status(app: AppState<'_>) -> PlayitStatus {
    app.playit.status()
}

/// Installs the agent and opens the claim link in the browser (F3).
#[tauri::command]
pub async fn playit_setup(app: AppState<'_>, handle: tauri::AppHandle) -> CmdResult<String> {
    let url = app.playit.setup().await.map_err(err)?;
    let _ = handle.opener().open_url(&url, None::<&str>);
    Ok(url)
}

#[tauri::command]
pub fn playit_cancel(app: AppState<'_>) {
    app.playit.cancel_setup();
}

#[tauri::command]
pub async fn playit_relink(app: AppState<'_>, handle: tauri::AppHandle) -> CmdResult<String> {
    let url = app.playit.relink().await.map_err(err)?;
    let _ = handle.opener().open_url(&url, None::<&str>);
    Ok(url)
}
