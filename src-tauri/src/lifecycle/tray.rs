//! System tray (R8, KTD14): Open and Quit. Closing the window hides it to the tray
//! when "close to tray" is on; Quit stops every server gracefully, then exits.

use std::sync::Arc;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
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
