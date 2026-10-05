use super::{err, AppState, CmdResult};
use crate::core::app::JavaRuntimeInfo;

#[tauri::command]
pub fn list_java_runtimes(app: AppState<'_>) -> Vec<JavaRuntimeInfo> {
    app.java_runtimes()
}

#[tauri::command]
pub fn remove_java_runtime(app: AppState<'_>, major: u32) -> CmdResult<()> {
    app.remove_java_runtime(major).map_err(err)
}
