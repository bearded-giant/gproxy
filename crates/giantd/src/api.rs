use crate::config::{self, AppConfig, ProfileMeta, ProfileRaw, RuleState};
use crate::events::{EventBus, ProxyEvent};
use crate::rules::Rule;
use axum::{
    extract::{ws::WebSocket, Path, State, WebSocketUpgrade},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<AppConfig>>,
    pub rules: Arc<RwLock<Vec<Rule>>>,
    pub active_profile: Arc<RwLock<Option<String>>>,
    pub event_bus: Arc<EventBus>,
    pub started_at: Arc<RwLock<Option<chrono::DateTime<chrono::Utc>>>>,
}

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/status", get(get_status))
        .route("/start", post(start_proxy))
        .route("/stop", post(stop_proxy))
        .route("/profiles", get(list_profiles))
        .route("/profiles/{name}", get(get_profile))
        .route("/use/{profile}", post(switch_profile))
        .route("/rules/{id}", get(get_rule))
        .route("/rules/{id}", put(update_rule))
        .route("/rules/{id}/toggle", post(toggle_rule))
        .route("/rules", post(add_rule))
        .route("/rules/{id}", delete(remove_rule))
        .route("/rules/reorder", post(reorder_rules))
        .route("/events", get(event_stream))
        .route("/logs", get(get_logs))
        .route("/env", get(get_env_snippet))
        .with_state(state)
}

async fn health_check() -> impl IntoResponse {
    Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "protocol_version": 1,
        "pid": std::process::id()
    }))
}

async fn get_status(State(state): State<AppState>) -> impl IntoResponse {
    let rules = state.rules.read().await;
    let profile = state.active_profile.read().await;
    let config = state.config.read().await;
    let started_at = state.started_at.read().await;

    let rule_states: Vec<RuleState> = rules
        .iter()
        .map(|r| RuleState {
            id: r.id.clone(),
            enabled: r.enabled,
            matched_count: 0,
        })
        .collect();

    Json(json!({
        "running": profile.is_some(),
        "profile": *profile,
        "rules": rule_states,
        "listen_addr": format!("127.0.0.1:{}", config.listen_port),
        "routing_mode": config.routing_mode,
        "started_at": started_at.map(|t| t.to_rfc3339()),
    }))
}

async fn list_profiles() -> impl IntoResponse {
    match config::list_profiles() {
        Ok(profiles) => Json(json!({ "profiles": profiles })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn switch_profile(
    State(state): State<AppState>,
    Path(profile_name): Path<String>,
) -> impl IntoResponse {
    match config::load_profile(&profile_name) {
        Ok(profile) => {
            let rules_loaded = profile.rules.len();
            *state.rules.write().await = profile.rules;
            *state.active_profile.write().await = Some(profile_name.clone());

            state.event_bus.send(ProxyEvent::ProfileSwitched {
                profile: profile_name,
                rules_loaded,
            });

            Json(json!({ "ok": true, "rules_loaded": rules_loaded })).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn toggle_rule(
    State(state): State<AppState>,
    Path(rule_id): Path<String>,
) -> impl IntoResponse {
    let mut rules = state.rules.write().await;
    if let Some(rule) = rules.iter_mut().find(|r| r.id == rule_id) {
        rule.enabled = !rule.enabled;
        let enabled = rule.enabled;

        state.event_bus.send(ProxyEvent::RuleToggled {
            rule_id: rule_id.clone(),
            enabled,
        });

        Json(json!({ "id": rule_id, "enabled": enabled })).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "rule not found" })),
        )
            .into_response()
    }
}

async fn get_profile(Path(name): Path<String>) -> impl IntoResponse {
    match config::load_profile(&name) {
        Ok(profile) => {
            let rules: Vec<serde_json::Value> = profile
                .rules
                .iter()
                .map(|r| {
                    json!({
                        "id": r.id,
                        "enabled": r.enabled,
                        "preserve_host": r.preserve_host,
                        "priority": r.priority,
                    })
                })
                .collect();
            Json(json!({
                "meta": profile.meta,
                "rules": rules,
            }))
            .into_response()
        }
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn start_proxy(
    State(state): State<AppState>,
    body: Option<Json<serde_json::Value>>,
) -> impl IntoResponse {
    let profile_name = match body
        .as_ref()
        .and_then(|b| b.get("profile"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
    {
        Some(name) => Some(name),
        None => state.config.read().await.default_profile.clone(),
    };

    let Some(name) = profile_name else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "no profile specified and no default configured"})),
        )
            .into_response();
    };

    match config::load_profile(&name) {
        Ok(profile) => {
            let rules_loaded = profile.rules.len();
            *state.rules.write().await = profile.rules;
            *state.active_profile.write().await = Some(name.clone());
            *state.started_at.write().await = Some(chrono::Utc::now());

            let config = state.config.read().await;
            state.event_bus.send(ProxyEvent::ProxyStarted {
                listen_addr: format!("127.0.0.1:{}", config.listen_port),
                profile: name,
            });

            Json(json!({"ok": true, "rules_loaded": rules_loaded})).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn stop_proxy(State(state): State<AppState>) -> impl IntoResponse {
    state.rules.write().await.clear();
    *state.active_profile.write().await = None;
    *state.started_at.write().await = None;
    state.event_bus.send(ProxyEvent::ProxyStopped);
    Json(json!({"ok": true}))
}

async fn get_rule(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let rules = state.rules.read().await;
    match rules.iter().find(|r| r.id == id) {
        Some(rule) => {
            let raw = rule.to_raw();
            Json(json!({
                "id": raw.id,
                "enabled": raw.enabled,
                "match_rule": raw.match_rule,
                "target": raw.target,
                "preserve_host": raw.preserve_host,
                "priority": raw.priority,
            }))
            .into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "rule not found"})),
        )
            .into_response(),
    }
}

async fn update_rule(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let mut rules = state.rules.write().await;
    let Some(idx) = rules.iter().position(|r| r.id == id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "rule not found"})),
        )
            .into_response();
    };

    let existing = &rules[idx];
    let raw = crate::rules::RuleRaw {
        id: body
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or(&existing.id)
            .to_string(),
        enabled: body
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(existing.enabled),
        match_rule: if let Some(mr) = body.get("match_rule") {
            serde_json::from_value(mr.clone()).unwrap_or_else(|_| existing.match_rule.clone())
        } else {
            existing.match_rule.clone()
        },
        target: if let Some(t) = body.get("target") {
            serde_json::from_value(t.clone()).unwrap_or_else(|_| existing.target.clone())
        } else {
            existing.target.clone()
        },
        preserve_host: body
            .get("preserve_host")
            .and_then(|v| v.as_bool())
            .unwrap_or(existing.preserve_host),
        priority: body
            .get("priority")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(existing.priority),
    };

    match Rule::from_raw(raw) {
        Ok(rule) => {
            rules[idx] = rule;
            drop(rules);
            persist_rules(&state).await;
            Json(json!({"ok": true})).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn add_rule(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let raw: crate::rules::RuleRaw = match serde_json::from_value(body) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };

    match Rule::from_raw(raw) {
        Ok(rule) => {
            let id = rule.id.clone();
            state.rules.write().await.push(rule);
            persist_rules(&state).await;
            Json(json!({"ok": true, "id": id})).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn remove_rule(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let mut rules = state.rules.write().await;
    let len_before = rules.len();
    rules.retain(|r| r.id != id);
    if rules.len() == len_before {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "rule not found"})),
        )
            .into_response();
    }
    drop(rules);
    persist_rules(&state).await;
    Json(json!({"ok": true})).into_response()
}

async fn reorder_rules(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let Some(order) = body.get("order").and_then(|v| v.as_array()) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "missing order array"})),
        )
            .into_response();
    };

    let order: Vec<String> = order
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect();
    let mut rules = state.rules.write().await;
    let mut sorted = Vec::with_capacity(rules.len());
    for id in &order {
        if let Some(pos) = rules.iter().position(|r| &r.id == id) {
            sorted.push(rules.remove(pos));
        }
    }
    sorted.append(&mut *rules);
    *rules = sorted;
    drop(rules);
    persist_rules(&state).await;
    Json(json!({"ok": true})).into_response()
}

async fn event_stream(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(mut socket: WebSocket, state: AppState) {
    let mut rx = state.event_bus.subscribe();
    while let Ok(event) = rx.recv().await {
        if let Ok(json) = serde_json::to_string(&event) {
            if socket
                .send(axum::extract::ws::Message::Text(json.into()))
                .await
                .is_err()
            {
                break;
            }
        }
    }
}

async fn get_logs() -> impl IntoResponse {
    Json(json!({"logs": []}))
}

async fn get_env_snippet(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.read().await;
    let ca_path = config::config_dir().join("ca").join("giant-proxy-ca.pem");
    let snippet =
        crate::routing::generate_env_snippet(config.listen_port, &ca_path, &config.bypass_hosts);
    Json(json!({ "shell_snippet": snippet }))
}

async fn persist_rules(state: &AppState) {
    let profile_name = state.active_profile.read().await.clone();
    let Some(name) = profile_name else { return };
    let rules = state.rules.read().await;
    let raw_rules: Vec<crate::rules::RuleRaw> = rules.iter().map(|r| r.to_raw()).collect();
    let profile_raw = ProfileRaw {
        meta: ProfileMeta {
            name: name.clone(),
            description: None,
            format_version: 1,
        },
        rules: raw_rules,
    };
    let _ = config::write_profile(&profile_raw);
}
