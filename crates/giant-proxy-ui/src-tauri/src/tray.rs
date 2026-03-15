use crate::daemon::DaemonClient;
use std::sync::Mutex;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

pub struct TrayState {
    pub status_item: tauri::menu::MenuItem<tauri::Wry>,
}

pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let status_item = MenuItemBuilder::with_id("status", "Giant Proxy not running")
        .enabled(false)
        .build(app)?;

    let start_item = MenuItemBuilder::with_id("start", "Start Proxy").build(app)?;
    let stop_item = MenuItemBuilder::with_id("stop", "Stop Proxy").build(app)?;

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

    app.manage(Mutex::new(TrayState {
        status_item: status_item.clone(),
    }));

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

    // poll daemon status and update tray text
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let text = get_tray_text().await;
            if let Some(state) = app_handle.try_state::<Mutex<TrayState>>() {
                if let Ok(s) = state.lock() {
                    let _ = s.status_item.set_text(&text);
                }
            }
        }
    });

    Ok(())
}

async fn get_tray_text() -> String {
    let client = DaemonClient::new();
    if !client.is_daemon_running() {
        return "Giant Proxy not running".to_string();
    }
    match client.get("/status").await {
        Ok(resp) => {
            let running = resp.get("running").and_then(|v| v.as_bool()).unwrap_or(false);
            let profile = resp.get("profile").and_then(|v| v.as_str()).unwrap_or("-");
            let rules = resp
                .get("rules")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter(|r| {
                            r.get("enabled")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false)
                        })
                        .count()
                })
                .unwrap_or(0);
            if running {
                format!("{} ({} rules)", profile, rules)
            } else {
                "Giant Proxy idle".to_string()
            }
        }
        Err(_) => "Giant Proxy not running".to_string(),
    }
}
