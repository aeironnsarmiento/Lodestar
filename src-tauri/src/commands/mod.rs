//! Typed Tauri commands. Each is a thin wrapper over [`crate::core::app::App`];
//! errors cross the boundary as user-readable strings.

use std::sync::Arc;

use crate::core::app::App;

pub mod instances;
pub mod settings;

pub type AppState<'a> = tauri::State<'a, Arc<App>>;
pub type CmdResult<T> = Result<T, String>;

pub fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}
