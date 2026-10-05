use std::path::PathBuf;

use super::{err, AppState, CmdResult};
use crate::addons::http::{ProjectVersion, SearchPage, SearchQuery};
use crate::addons::modpack::PackInfo;
use crate::addons::service::{AddonUpdate, InstallResult};
use crate::addons::{AddonEntry, AddonSource, ImportResult, ProjectKind};

#[tauri::command]
pub fn list_addons(app: AppState<'_>, id: String) -> CmdResult<Vec<AddonEntry>> {
    app.list_addons(&id).map_err(err)
}

#[tauri::command]
pub async fn identify_addons(app: AppState<'_>, id: String) -> CmdResult<Vec<AddonEntry>> {
    app.identify_addons(&id).await.map_err(err)
}

#[tauri::command]
pub fn set_addon_enabled(app: AppState<'_>, id: String, file_name: String, enabled: bool) -> CmdResult<()> {
    app.set_addon_enabled(&id, &file_name, enabled).map_err(err)
}

#[tauri::command]
pub fn remove_addon(app: AppState<'_>, id: String, file_name: String) -> CmdResult<()> {
    app.remove_addon(&id, &file_name).map_err(err)
}

#[tauri::command]
pub fn import_addons(app: AppState<'_>, id: String, paths: Vec<PathBuf>) -> CmdResult<ImportResult> {
    app.import_addons(&id, &paths).map_err(err)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn search_projects(
    app: AppState<'_>,
    source: AddonSource,
    kind: ProjectKind,
    text: String,
    game_version: Option<String>,
    loaders: Vec<String>,
    offset: u32,
) -> CmdResult<SearchPage> {
    let query = SearchQuery { text, game_version, loaders, offset, limit: 20 };
    app.search_projects(source, kind, query).await.map_err(err)
}

#[tauri::command]
pub async fn project_versions(
    app: AppState<'_>,
    source: AddonSource,
    project_id: String,
    loaders: Vec<String>,
    game_version: Option<String>,
) -> CmdResult<Vec<ProjectVersion>> {
    app.project_versions(source, &project_id, &loaders, game_version.as_deref()).await.map_err(err)
}

#[tauri::command]
pub async fn install_project(
    app: AppState<'_>,
    id: String,
    source: AddonSource,
    project_id: String,
    version_id: Option<String>,
) -> CmdResult<InstallResult> {
    app.install_project(&id, source, &project_id, version_id.as_deref()).await.map_err(err)
}

#[tauri::command]
pub async fn check_addon_updates(app: AppState<'_>, id: String) -> CmdResult<Vec<AddonUpdate>> {
    app.check_addon_updates(&id).await.map_err(err)
}

#[tauri::command]
pub async fn update_addon(app: AppState<'_>, id: String, file_name: String, version_id: String) -> CmdResult<InstallResult> {
    app.update_addon(&id, &file_name, &version_id).await.map_err(err)
}

#[tauri::command]
pub fn inspect_modpack_file(app: AppState<'_>, path: PathBuf) -> CmdResult<PackInfo> {
    app.inspect_modpack_file(&path).map_err(err)
}

/// Async so the provisioning task can be spawned on the runtime.
#[tauri::command]
pub async fn update_modpack(app: AppState<'_>, id: String, version_id: String, version_number: Option<String>) -> CmdResult<()> {
    app.inner().update_modpack(&id, &version_id, version_number).map_err(err)
}
