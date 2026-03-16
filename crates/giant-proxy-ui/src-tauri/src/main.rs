#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod daemon;
mod tray;

use daemon::DaemonClient;
use tauri::{Emitter, Manager};
use tauri_plugin_autostart::MacosLauncher;

#[tauri::command]
async fn get_status() -> Result<serde_json::Value, String> {
    let client = DaemonClient::new();
    client.get("/status").await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_profiles() -> Result<serde_json::Value, String> {
    let client = DaemonClient::new();
    client.get("/profiles").await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_rule(rule_id: String) -> Result<serde_json::Value, String> {
    let client = DaemonClient::new();
    client
        .post(&format!("/rules/{}/toggle", rule_id), None)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn switch_profile(profile: String) -> Result<serde_json::Value, String> {
    let client = DaemonClient::new();
    client
        .post(&format!("/use/{}", profile), None)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_proxy() -> Result<serde_json::Value, String> {
    let client = DaemonClient::new();
    client.post("/start", None).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn stop_proxy() -> Result<serde_json::Value, String> {
    let client = DaemonClient::new();
    client.post("/stop", None).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_env() -> Result<serde_json::Value, String> {
    let client = DaemonClient::new();
    client.get("/env").await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_rule(rule_id: String) -> Result<serde_json::Value, String> {
    let client = DaemonClient::new();
    client
        .get(&format!("/rules/{}", rule_id))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_rule(rule: serde_json::Value) -> Result<serde_json::Value, String> {
    let client = DaemonClient::new();
    client
        .post("/rules", Some(rule))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_rule(
    rule_id: String,
    rule: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let client = DaemonClient::new();
    client
        .put(&format!("/rules/{}", rule_id), rule)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_rule(rule_id: String) -> Result<serde_json::Value, String> {
    let client = DaemonClient::new();
    client
        .delete(&format!("/rules/{}", rule_id))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn reorder_rules(order: Vec<String>) -> Result<serde_json::Value, String> {
    let client = DaemonClient::new();
    let body = serde_json::json!({"order": order});
    client
        .post("/rules/reorder", Some(body))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_profiles_local() -> Result<serde_json::Value, String> {
    let names = giantd::config::list_profiles().map_err(|e| e.to_string())?;
    let mut profiles = Vec::new();
    for name in &names {
        match giantd::config::load_profile(name) {
            Ok(profile) => {
                let rules: Vec<serde_json::Value> = profile.rules.iter().map(|r| {
                    let match_display = r.match_rule.regex.as_deref()
                        .or(r.match_rule.host.as_deref())
                        .unwrap_or("-");
                    serde_json::json!({
                        "id": r.id,
                        "enabled": r.enabled,
                        "match_display": match_display,
                        "target": format!("{}://{}:{}", r.target.scheme, r.target.host, r.target.port),
                    })
                }).collect();
                profiles.push(serde_json::json!({
                    "name": name,
                    "description": profile.meta.description,
                    "rules": rules,
                }));
            }
            Err(e) => {
                tracing::error!("failed to load profile '{}': {}", name, e);
                profiles.push(serde_json::json!({
                    "name": name,
                    "rules": [],
                    "error": e.to_string(),
                }));
            }
        }
    }
    Ok(serde_json::json!({ "profiles": profiles }))
}

#[tauri::command]
async fn rename_profile(old_name: String, new_name: String) -> Result<serde_json::Value, String> {
    giantd::config::rename_profile(&old_name, &new_name).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"ok": true}))
}

#[tauri::command]
async fn reorder_profiles(names: Vec<String>) -> Result<serde_json::Value, String> {
    giantd::config::save_profile_order(&names).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"ok": true}))
}

#[tauri::command]
async fn save_profile_rule(
    profile_name: String,
    rule: serde_json::Value,
    old_rule_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut profile_raw = match giantd::config::load_profile(&profile_name) {
        Ok(p) => giantd::config::ProfileRaw {
            meta: p.meta,
            rules: p.rules.iter().map(|r| r.to_raw()).collect(),
        },
        Err(e) => return Err(e.to_string()),
    };

    let new_rule: giantd::rules::RuleRaw =
        serde_json::from_value(rule).map_err(|e| e.to_string())?;

    if let Some(old_id) = old_rule_id {
        // update existing rule
        if let Some(r) = profile_raw.rules.iter_mut().find(|r| r.id == old_id) {
            *r = new_rule;
        } else {
            return Err(format!("rule '{}' not found in profile", old_id));
        }
    } else {
        // add new rule
        profile_raw.rules.push(new_rule);
    }

    giantd::config::write_profile(&profile_raw).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"ok": true}))
}

#[tauri::command]
async fn delete_profile_rule(
    profile_name: String,
    rule_id: String,
) -> Result<serde_json::Value, String> {
    let mut profile_raw = match giantd::config::load_profile(&profile_name) {
        Ok(p) => giantd::config::ProfileRaw {
            meta: p.meta,
            rules: p.rules.iter().map(|r| r.to_raw()).collect(),
        },
        Err(e) => return Err(e.to_string()),
    };

    let before = profile_raw.rules.len();
    profile_raw.rules.retain(|r| r.id != rule_id);
    if profile_raw.rules.len() == before {
        return Err(format!("rule '{}' not found", rule_id));
    }

    giantd::config::write_profile(&profile_raw).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"ok": true}))
}

#[tauri::command]
async fn toggle_profile_rule(
    profile_name: String,
    rule_id: String,
) -> Result<serde_json::Value, String> {
    let mut profile_raw = match giantd::config::load_profile(&profile_name) {
        Ok(p) => giantd::config::ProfileRaw {
            meta: p.meta,
            rules: p.rules.iter().map(|r| r.to_raw()).collect(),
        },
        Err(e) => return Err(e.to_string()),
    };

    if let Some(r) = profile_raw.rules.iter_mut().find(|r| r.id == rule_id) {
        r.enabled = !r.enabled;
    } else {
        return Err(format!("rule '{}' not found", rule_id));
    }

    giantd::config::write_profile(&profile_raw).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"ok": true}))
}

#[tauri::command]
async fn get_profile_rule(
    profile_name: String,
    rule_id: String,
) -> Result<serde_json::Value, String> {
    let profile = giantd::config::load_profile(&profile_name).map_err(|e| e.to_string())?;
    let rule = profile.rules.iter().find(|r| r.id == rule_id)
        .ok_or_else(|| format!("rule '{}' not found", rule_id))?;
    let raw = rule.to_raw();
    Ok(serde_json::json!({
        "id": raw.id,
        "enabled": raw.enabled,
        "match_rule": raw.match_rule,
        "target": raw.target,
        "preserve_host": raw.preserve_host,
        "priority": raw.priority,
    }))
}

#[tauri::command]
async fn start_daemon() -> Result<serde_json::Value, String> {
    let client = DaemonClient::new();
    if client.is_daemon_running() {
        return Ok(serde_json::json!({"ok": true, "already_running": true}));
    }
    client.ensure_daemon_started().await?;
    Ok(serde_json::json!({"ok": true}))
}

#[tauri::command]
async fn stop_daemon() -> Result<serde_json::Value, String> {
    let client = DaemonClient::new();
    let _ = client.post("/stop", None).await;
    let config_dir = dirs::home_dir()
        .expect("home dir")
        .join(".giant-proxy");
    if let Ok(Some(pid)) = giantd::pid::read_pid(&config_dir) {
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();
    }
    Ok(serde_json::json!({"ok": true}))
}

#[tauri::command]
async fn get_launch_at_login(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_launch_at_login(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())
    } else {
        manager.disable().map_err(|e| e.to_string())
    }
}

#[tauri::command]
async fn import_proxyman_file(file_path: String) -> Result<serde_json::Value, String> {
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(format!("file not found: {}", file_path));
    }

    let profiles = giantd::convert::import_proxyman(path).map_err(|e| e.to_string())?;
    let mut imported = Vec::new();
    for (name, profile) in &profiles {
        giantd::convert::save_profile(profile).map_err(|e| e.to_string())?;
        imported.push(serde_json::json!({
            "name": name,
            "rules": profile.rules.len(),
        }));
    }

    Ok(serde_json::json!({
        "ok": true,
        "profiles": imported,
    }))
}

#[tauri::command]
async fn import_proxyman_auto() -> Result<serde_json::Value, String> {
    let path = dirs::home_dir()
        .expect("home dir")
        .join("Library/Application Support/com.proxyman.NSProxy/user-data/MapRemoteService");

    if !path.exists() {
        return Err("Proxyman not found. No Map Remote config at ~/Library/Application Support/com.proxyman.NSProxy/".to_string());
    }

    let profiles = giantd::convert::import_proxyman(&path).map_err(|e| e.to_string())?;
    let mut imported = Vec::new();
    for (name, profile) in &profiles {
        giantd::convert::save_profile(profile).map_err(|e| e.to_string())?;
        imported.push(serde_json::json!({
            "name": name,
            "rules": profile.rules.len(),
        }));
    }

    Ok(serde_json::json!({
        "ok": true,
        "profiles": imported,
    }))
}

#[tauri::command]
async fn open_dashboard(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("dashboard") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;

        #[cfg(target_os = "macos")]
        {
            let _ = app.run_on_main_thread(|| {
                use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
                use objc2::MainThreadMarker;
                let mtm = MainThreadMarker::new().expect("run_on_main_thread");
                let ns_app = NSApplication::sharedApplication(mtm);
                ns_app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
            });
        }
    }
    Ok(())
}

#[tauri::command]
async fn check_for_update(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let current = app.config().version.clone().unwrap_or_default();
    let current_ver = semver::Version::parse(&current).unwrap_or(semver::Version::new(0, 0, 0));

    let client = reqwest::Client::builder()
        .user_agent("giant-proxy-update-check")
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get("https://api.github.com/repos/bearded-giant/gproxy/releases/latest")
        .send()
        .await
        .map_err(|e| format!("failed to check: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("github returned {}", resp.status()));
    }

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let tag = body
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let latest_str = tag.strip_prefix('v').unwrap_or(tag);
    let latest_ver = semver::Version::parse(latest_str).unwrap_or(semver::Version::new(0, 0, 0));
    let html_url = body
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    Ok(serde_json::json!({
        "current": current,
        "latest": latest_str,
        "update_available": latest_ver > current_ver,
        "url": html_url,
    }))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
                use objc2::MainThreadMarker;
                let mtm = MainThreadMarker::new().expect("must be on main thread");
                let ns_app = NSApplication::sharedApplication(mtm);
                ns_app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
            }

            // first-launch setup: config dirs, CA cert, launch-at-login
            {
                use tauri_plugin_autostart::ManagerExt;
                let config_dir = dirs::home_dir()
                    .expect("home dir")
                    .join(".giant-proxy");
                let marker = config_dir.join(".ui-initialized");

                let _ = std::fs::create_dir_all(&config_dir);
                let _ = std::fs::create_dir_all(config_dir.join("profiles"));
                let _ = std::fs::create_dir_all(config_dir.join("logs"));

                // generate CA cert if missing
                let ca_cert = config_dir.join("ca").join("giant-proxy-ca.pem");
                if !ca_cert.exists() {
                    match giantd::certs::CertAuthority::generate(&config_dir) {
                        Ok(ca) => {
                            tracing::info!("generated CA cert");
                            // install to trust store (prompts for password on macOS)
                            if let Err(e) = ca.install_trust_store() {
                                tracing::warn!("CA trust install failed (user can run `giant-proxy init`): {}", e);
                            }
                        }
                        Err(e) => tracing::error!("failed to generate CA: {}", e),
                    }
                }

                if !marker.exists() {
                    let _ = app.autolaunch().enable();

                    // first launch: symlink CLI binaries to /usr/local/bin
                    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) {
                        let mut cmds = Vec::new();
                        for bin in &["giant-proxy", "giantd"] {
                            let src = exe_dir.join(bin);
                            let dest = format!("/usr/local/bin/{}", bin);
                            if src.exists() {
                                cmds.push(format!("ln -sf '{}' '{}'", src.display(), dest));
                            }
                        }
                        if !cmds.is_empty() {
                            let script = format!(
                                "do shell script \"{}\" with administrator privileges with prompt \"Giant Proxy wants to install CLI commands (giant-proxy, giantd) to /usr/local/bin\"",
                                cmds.join(" && ")
                            );
                            match std::process::Command::new("osascript")
                                .args(["-e", &script])
                                .status()
                            {
                                Ok(s) if s.success() => tracing::info!("CLI symlinked to /usr/local/bin"),
                                _ => tracing::warn!("CLI symlink skipped"),
                            }
                        }
                    }

                    let _ = std::fs::write(&marker, "");
                }
            }

            // hide dashboard on close instead of destroying, toggle dock icon
            if let Some(dashboard) = app.get_webview_window("dashboard") {
                let dh = dashboard.clone();
                dashboard.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = dh.hide();

                        #[cfg(target_os = "macos")]
                        {
                            use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
                            use objc2::MainThreadMarker;
                            if let Some(mtm) = MainThreadMarker::new() {
                                let ns_app = NSApplication::sharedApplication(mtm);
                                ns_app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
                            }
                        }
                    }
                });
            }

            tray::setup_tray(app)?;

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                event_forwarder(app_handle).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_profiles,
            toggle_rule,
            switch_profile,
            start_proxy,
            stop_proxy,
            get_env,
            get_rule,
            add_rule,
            update_rule,
            delete_rule,
            reorder_rules,
            open_dashboard,
            get_launch_at_login,
            set_launch_at_login,
            import_proxyman_file,
            list_profiles_local,
            save_profile_rule,
            delete_profile_rule,
            toggle_profile_rule,
            get_profile_rule,
            start_daemon,
            stop_daemon,
            import_proxyman_auto,
            rename_profile,
            reorder_profiles,
            check_for_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn event_forwarder(app: tauri::AppHandle) {
    use futures_util::StreamExt;

    let mut backoff_ms = 1000u64;

    loop {
        let client = DaemonClient::new();

        if !client.is_daemon_running() {
            tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
            backoff_ms = (backoff_ms * 2).min(30_000);
            continue;
        }

        match client.connect_events().await {
            Ok(mut ws) => {
                backoff_ms = 1000;
                let _ = app.emit("ws-status", serde_json::json!({"connected": true}));

                while let Some(msg) = ws.next().await {
                    match msg {
                        Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                            if let Ok(event) =
                                serde_json::from_str::<serde_json::Value>(text.as_ref())
                            {
                                let _ = app.emit("proxy-event", &event);
                            }
                        }
                        Err(_) => break,
                        _ => {}
                    }
                }

                let _ = app.emit("ws-status", serde_json::json!({"connected": false}));
            }
            Err(_) => {
                let _ = app.emit("ws-status", serde_json::json!({"connected": false}));
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
        backoff_ms = (backoff_ms * 2).min(30_000);
    }
}
