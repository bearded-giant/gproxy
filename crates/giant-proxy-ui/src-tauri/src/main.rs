#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod daemon;
mod tray;

use daemon::DaemonClient;
use tauri::{Emitter, Manager};

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
async fn open_dashboard(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("dashboard") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
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
