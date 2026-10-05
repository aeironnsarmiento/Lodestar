use super::{err, AppState, CmdResult};
use crate::core::instance::{Instance, NewInstance, ServerType};
use crate::providers::VersionEntry;

#[tauri::command]
pub fn list_instances(app: AppState<'_>) -> Vec<Instance> {
    app.list_instances()
}

#[tauri::command]
pub fn create_instance(app: AppState<'_>, new: NewInstance) -> CmdResult<Instance> {
    app.create_instance(new).map_err(err)
}

#[tauri::command]
pub fn update_instance(app: AppState<'_>, instance: Instance) -> CmdResult<Instance> {
    app.update_instance(instance).map_err(err)
}

#[tauri::command]
pub fn delete_instance(app: AppState<'_>, id: String) -> CmdResult<()> {
    app.delete_instance(&id).map_err(err)
}

#[tauri::command]
pub async fn list_versions(app: AppState<'_>, server_type: ServerType) -> CmdResult<Vec<VersionEntry>> {
    app.versions(server_type).await.map_err(err)
}
