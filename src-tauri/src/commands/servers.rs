use super::{err, AppState, CmdResult};
use crate::core::control::JoinInfo;
use crate::supervisor::console::ConsoleLine;
use crate::supervisor::Snapshot;

#[tauri::command]
pub async fn start_server(app: AppState<'_>, id: String) -> CmdResult<()> {
    app.inner().clone().launch(&id).await.map_err(err)
}

// Async so it runs on the tokio runtime: stopping arms a force-kill timer task.
#[tauri::command]
pub async fn stop_server(app: AppState<'_>, id: String) -> CmdResult<()> {
    app.inner().stop_server(&id).map_err(err)
}

#[tauri::command]
pub async fn restart_server(app: AppState<'_>, id: String) -> CmdResult<()> {
    app.inner().clone().restart_server(&id).await.map_err(err)
}

#[tauri::command]
pub fn kill_server(app: AppState<'_>, id: String) -> CmdResult<()> {
    app.kill_server(&id).map_err(err)
}

#[tauri::command]
pub fn send_command(app: AppState<'_>, id: String, command: String) -> CmdResult<()> {
    app.send_command(&id, &command).map_err(err)
}

#[tauri::command]
pub fn get_console(app: AppState<'_>, id: String) -> Vec<ConsoleLine> {
    app.console(&id)
}

#[tauri::command]
pub fn server_snapshots(app: AppState<'_>) -> Vec<Snapshot> {
    app.snapshots()
}

#[tauri::command]
pub fn join_info(app: AppState<'_>, id: String) -> CmdResult<JoinInfo> {
    app.join_info(&id).map_err(err)
}
