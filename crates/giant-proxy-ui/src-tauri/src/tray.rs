use crate::daemon::DaemonClient;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let status_item = MenuItemBuilder::with_id("status", "Giant Proxy (stopped)")
        .enabled(false)
        .build(app)?;

    let start_item = MenuItemBuilder::with_id("start", "Start Proxy").build(app)?;
    let stop_item = MenuItemBuilder::with_id("stop", "Stop Proxy").build(app)?;

    let profile_submenu = SubmenuBuilder::with_id(app, "profiles", "Switch Profile")
        .text("profile_preprod", "preprod")
        .text("profile_stage", "stage")
        .text("profile_prod", "prod")
        .build()?;

    let copy_env_item = MenuItemBuilder::with_id("copy_env", "Copy Proxy Env...").build(app)?;
    let dashboard_item = MenuItemBuilder::with_id("dashboard", "Open Dashboard...").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&status_item)
        .separator()
        .item(&start_item)
        .item(&stop_item)
        .separator()
        .item(&profile_submenu)
        .separator()
        .item(&copy_env_item)
        .item(&dashboard_item)
        .separator()
        .item(&quit_item)
        .build()?;

    TrayIconBuilder::new()
        .tooltip("Giant Proxy")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            match id {
                "start" => {
                    tauri::async_runtime::spawn(async move {
                        let client = DaemonClient::new();
                        let _ = client.post("/start", None).await;
                    });
                }
                "stop" => {
                    tauri::async_runtime::spawn(async move {
                        let client = DaemonClient::new();
                        let _ = client.post("/stop", None).await;
                    });
                }
                "copy_env" => {
                    let handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let client = DaemonClient::new();
                        if let Ok(resp) = client.get("/env").await {
                            if let Some(snippet) =
                                resp.get("shell_snippet").and_then(|s| s.as_str())
                            {
                                // emit to frontend for clipboard copy
                                let _ = handle.emit("copy-to-clipboard", snippet);
                            }
                        }
                    });
                }
                "dashboard" => {
                    if let Some(window) = app.get_webview_window("dashboard") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ if id.starts_with("profile_") => {
                    if let Some(profile) = id.strip_prefix("profile_") {
                        let profile = profile.to_string();
                        tauri::async_runtime::spawn(async move {
                            let client = DaemonClient::new();
                            let _ = client.post(&format!("/use/{}", profile), None).await;
                        });
                    }
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("popover") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
