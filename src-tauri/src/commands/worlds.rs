use super::{err, AppState, CmdResult};
use crate::worlds::WorldInfo;

#[tauri::command]
pub fn list_worlds(app: AppState<'_>, id: String) -> CmdResult<Vec<WorldInfo>> {
    app.worlds(&id).map_err(err)
}

#[tauri::command]
pub async fn reset_world(app: AppState<'_>, id: String, seed: Option<String>) -> CmdResult<String> {
    app.inner().clone().reset_world(&id, seed).await.map_err(err)
}

#[tauri::command]
pub fn switch_world(app: AppState<'_>, id: String, name: String) -> CmdResult<()> {
    app.switch_world(&id, &name).map_err(err)
}
