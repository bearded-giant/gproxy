use crate::daemon::DaemonClient;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let status_item = MenuItemBuilder::with_id("status", "Giant Proxy not running")
        .enabled(false)
        .build(app)?;

    let start_item = MenuItemBuilder::with_id("start", "Start Proxy").build(app)?;
    let stop_item = MenuItemBuilder::with_id("stop", "Stop Proxy").build(app)?;

    // build profile submenu dynamically from disk
    let mut profile_sub = SubmenuBuilder::with_id(app, "profiles", "Switch Profile");
    match giantd::config::list_profiles() {
        Ok(profiles) => {
            if profiles.is_empty() {
                profile_sub = profile_sub.text("no_profiles", "(no profiles)");
            } else {
                for name in profiles {
                    let id = format!("profile_{}", name);
                    profile_sub = profile_sub.text(id, &name);
                }
            }
        }
        Err(_) => {
            profile_sub = profile_sub.text("no_profiles", "(no profiles)");
        }
    }
    let profile_submenu = profile_sub.build()?;

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
        .item(&dashboard_item)
        .separator()
        .item(&quit_item)
        .build()?;

    TrayIconBuilder::new()
        .icon(tauri::include_image!("icons/tray.png"))
        .icon_as_template(true)
        .tooltip("Giant Proxy")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            match id {
                "start" => {
                    tauri::async_runtime::spawn(async move {
                        let client = DaemonClient::new();
                        if let Err(e) = client.ensure_daemon_started().await {
                            tracing::error!("tray start: {}", e);
                            return;
                        }
                        let _ = client.post("/start", None).await;
                    });
                }
                "stop" => {
                    tauri::async_runtime::spawn(async move {
                        let client = DaemonClient::new();
                        let _ = client.post("/stop", None).await;
                    });
                }
                "dashboard" => {
                    if let Some(window) = app.get_webview_window("dashboard") {
                        let _ = window.show();
                        let _ = window.set_focus();

                        #[cfg(target_os = "macos")]
                        {
                            use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
                            use objc2::MainThreadMarker;
                            if let Some(mtm) = MainThreadMarker::new() {
                                let ns_app = NSApplication::sharedApplication(mtm);
                                ns_app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
                            }
                        }
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
                            if let Err(e) = client.ensure_daemon_started().await {
                                tracing::error!("tray profile switch: {}", e);
                                return;
                            }
                            let _ = client.post(&format!("/use/{}", profile), None).await;
                        });
                    }
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
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
            _ => {}
        })
        .build(app)?;

    Ok(())
}
