//! System tray (R8, KTD14): Open and Quit. Closing the window hides it to the tray
//! (the menu bar on macOS) when "close to tray" is on; Quit stops every server
//! gracefully, then exits.

use std::sync::Arc;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
#[cfg(target_os = "macos")]
use tauri::{menu::Submenu, Wry};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::core::app::App;

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Lodestar", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit (stops all servers)", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit_item])?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Lodestar")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "quit" => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Menu id of the app menu's Quit, distinct from the tray's so one click quits once.
pub const APP_QUIT: &str = "app-quit";

/// The macOS menu bar. Replacing Tauri's default keeps the Edit menu (copy and paste
/// in the webview need it) but routes ⌘Q through [`quit`], so servers stop cleanly.
#[cfg(target_os = "macos")]
pub fn app_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let quit = MenuItem::with_id(app, APP_QUIT, "Quit Lodestar", true, Some("CmdOrCtrl+Q"))?;
    let sep = || PredefinedMenuItem::separator(app);
    let lodestar = Submenu::with_items(
        app,
        "Lodestar",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("About Lodestar"), None)?,
            &sep()?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &sep()?,
            &quit,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &sep()?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[&PredefinedMenuItem::minimize(app, None)?, &PredefinedMenuItem::close_window(app, None)?],
    )?;
    Menu::with_items(app, &[&lodestar, &edit, &window])
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Stops all servers (and the playit agent), then exits.
pub fn quit(app: &AppHandle) {
    let handle = app.clone();
    let state = app.state::<Arc<App>>().inner().clone();
    tauri::async_runtime::spawn(async move {
        state.shutdown().await;
        handle.exit(0);
    });
}
