use axum::body::Body;
use axum::Router;
use giantd::api::{self, AppState};
use giantd::config::AppConfig;
use giantd::events::EventBus;
use http::Request;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceExt;

fn test_state() -> AppState {
    AppState {
        config: Arc::new(RwLock::new(AppConfig {
            version: 1,
            listen_port: 9456,
            pac_port: 9876,
            log_level: "info".to_string(),
            log_max_size_mb: 10,
            log_max_files: 5,
            auto_start: false,
            default_profile: None,
            routing_mode: "manual".to_string(),
            browser: None,
            bypass_hosts: vec![],
        })),
        rules: Arc::new(RwLock::new(Vec::new())),
        active_profile: Arc::new(RwLock::new(None)),
        event_bus: Arc::new(EventBus::new(16)),
        started_at: Arc::new(RwLock::new(None)),
        proxy_services: Arc::new(RwLock::new(Vec::new())),
    }
}

fn test_app() -> Router {
    api::routes(test_state())
}

async fn response_json(resp: axum::response::Response) -> (u16, serde_json::Value) {
    let status = resp.status().as_u16();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::json!({
        "_raw": String::from_utf8_lossy(&body).to_string(),
    }));
    (status, json)
}

// -- empty body handling (the axum 0.8 bug) --

#[tokio::test]
async fn post_with_empty_body_no_content_type_returns_json() {
    // reproduces the fix: clients must NOT send content-type: application/json
    // with an empty body, or axum 0.8 rejects before the handler runs
    let app = test_app();
    let req = Request::builder()
        .method("POST")
        .uri("/stop")
        .body(Body::empty())
        .unwrap();

    let (status, json) = response_json(app.oneshot(req).await.unwrap()).await;
    assert_eq!(status, 200);
    assert_eq!(json["ok"], true);
}

#[tokio::test]
async fn post_with_content_type_and_empty_body_rejected() {
    // documents the axum 0.8 behavior that caused the bug:
    // content-type: application/json + empty body = 400, not handler
    let app = test_app();
    let req = Request::builder()
        .method("POST")
        .uri("/start")
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    // axum rejects with 400 and plain text -- handler never runs
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn post_with_json_body_works() {
    let app = test_app();
    let body = serde_json::json!({"profile": "nonexistent"});
    let req = Request::builder()
        .method("POST")
        .uri("/start")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let (status, json) = response_json(app.oneshot(req).await.unwrap()).await;
    // profile doesn't exist on disk in test, but the handler ran (not a 400 parse error)
    assert!(status == 200 || status == 400);
    // if 400, it's because the profile doesn't exist on disk, not because of body parsing
    if status == 400 {
        let err = json["error"].as_str().unwrap();
        assert!(
            err.contains("No such file") || err.contains("not found") || err.contains("profile"),
            "expected filesystem/profile error, got: {}",
            json
        );
    }
}

// -- health / status --

#[tokio::test]
async fn health_returns_ok() {
    let app = test_app();
    let req = Request::builder()
        .uri("/health")
        .body(Body::empty())
        .unwrap();

    let (status, json) = response_json(app.oneshot(req).await.unwrap()).await;
    assert_eq!(status, 200);
    assert_eq!(json["ok"], true);
    assert!(json["version"].as_str().is_some());
    assert!(json["pid"].as_u64().is_some());
}

#[tokio::test]
async fn status_no_profile_shows_not_running() {
    let app = test_app();
    let req = Request::builder()
        .uri("/status")
        .body(Body::empty())
        .unwrap();

    let (status, json) = response_json(app.oneshot(req).await.unwrap()).await;
    assert_eq!(status, 200);
    assert_eq!(json["running"], false);
    assert!(json["profile"].is_null());
}

// -- stop --

#[tokio::test]
async fn stop_clears_state() {
    let state = test_state();
    *state.active_profile.write().await = Some("test".to_string());
    let app = api::routes(state.clone());

    let req = Request::builder()
        .method("POST")
        .uri("/stop")
        .body(Body::empty())
        .unwrap();

    let (status, json) = response_json(app.oneshot(req).await.unwrap()).await;
    assert_eq!(status, 200);
    assert_eq!(json["ok"], true);
    assert!(state.active_profile.read().await.is_none());
    assert!(state.rules.read().await.is_empty());
}

// -- rules CRUD (in-memory, no profile on disk) --

#[tokio::test]
async fn toggle_nonexistent_rule_returns_404() {
    let app = test_app();
    let req = Request::builder()
        .method("POST")
        .uri("/rules/fake_id/toggle")
        .body(Body::empty())
        .unwrap();

    let (status, json) = response_json(app.oneshot(req).await.unwrap()).await;
    assert_eq!(status, 404);
    assert!(json["error"].as_str().unwrap().contains("not found"));
}

#[tokio::test]
async fn add_and_toggle_rule() {
    let state = test_state();
    *state.active_profile.write().await = Some("test".to_string());
    let app = api::routes(state.clone());

    // add a rule
    let rule = serde_json::json!({
        "id": "test_rule",
        "enabled": true,
        "match": { "host": "*.example.com" },
        "target": { "host": "localhost", "port": 3000, "scheme": "http" },
        "preserve_host": true,
        "priority": 0,
    });
    let req = Request::builder()
        .method("POST")
        .uri("/rules")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&rule).unwrap()))
        .unwrap();
    let (status, _) = response_json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, 200);

    // verify it exists
    assert_eq!(state.rules.read().await.len(), 1);
    assert!(state.rules.read().await[0].enabled);

    // toggle it off
    let req = Request::builder()
        .method("POST")
        .uri("/rules/test_rule/toggle")
        .body(Body::empty())
        .unwrap();
    let (status, json) = response_json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, 200);
    assert_eq!(json["enabled"], false);
    assert!(!state.rules.read().await[0].enabled);
}

#[tokio::test]
async fn get_nonexistent_rule_returns_404() {
    let app = test_app();
    let req = Request::builder()
        .uri("/rules/nope")
        .body(Body::empty())
        .unwrap();

    let (status, _) = response_json(app.oneshot(req).await.unwrap()).await;
    assert_eq!(status, 404);
}

#[tokio::test]
async fn delete_nonexistent_rule_returns_404() {
    let app = test_app();
    let req = Request::builder()
        .method("DELETE")
        .uri("/rules/nope")
        .body(Body::empty())
        .unwrap();

    let (status, _) = response_json(app.oneshot(req).await.unwrap()).await;
    assert_eq!(status, 404);
}
