use super::{err, AppState, CmdResult};
use crate::core::instance::{Instance, NewInstance, ServerType};
use crate::core::loader::LoaderChoices;
use crate::providers::VersionEntry;

#[tauri::command]
pub fn list_instances(app: AppState<'_>) -> Vec<Instance> {
    app.list_instances()
}

/// Creates the record and provisions it in the background (async: spawns the task).
#[tauri::command]
pub async fn create_instance(app: AppState<'_>, new: NewInstance) -> CmdResult<Instance> {
    app.inner().create_and_provision(new).map_err(err)
}

#[tauri::command]
pub async fn retry_provision(app: AppState<'_>, id: String) -> CmdResult<()> {
    app.inner().retry_provision(&id).map_err(err)
}

#[tauri::command]
pub fn open_addons_folder(app: AppState<'_>, handle: tauri::AppHandle, id: String) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir = app.addons_folder(&id).map_err(err)?;
    handle
        .opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
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

/// Loader builds for a type and Minecraft version, checked against server `id`'s mods.
#[tauri::command]
pub async fn loader_choices(app: AppState<'_>, server_type: ServerType, mc_version: String, id: Option<String>) -> CmdResult<LoaderChoices> {
    app.loader_choices(server_type, &mc_version, id.as_deref()).await.map_err(err)
}

/// Pins a loader build (`null` = Automatic) and reinstalls the server software.
#[tauri::command]
pub async fn set_loader_version(app: AppState<'_>, id: String, version: Option<String>) -> CmdResult<()> {
    app.inner().set_loader_version(&id, version).map_err(err)
}
