use crate::events::{ProxyEvent, TrafficRecord};
use crate::rules::Rule;
use crate::traffic;
use hudsucker::{
    hyper::{Request, Response},
    Body, HttpContext, HttpHandler, RequestOrResponse,
};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

#[derive(Clone)]
pub struct ProxyHandler {
    pub rules: Arc<RwLock<Vec<Rule>>>,
    pub event_tx: broadcast::Sender<ProxyEvent>,
    pub traffic_buf: Arc<RwLock<traffic::TrafficBuffer>>,
    pending: Option<PendingTraffic>,
}

impl ProxyHandler {
    pub fn new(
        rules: Arc<RwLock<Vec<Rule>>>,
        event_tx: broadcast::Sender<ProxyEvent>,
        traffic_buf: Arc<RwLock<traffic::TrafficBuffer>>,
    ) -> Self {
        Self {
            rules,
            event_tx,
            traffic_buf,
            pending: None,
        }
    }
}

impl HttpHandler for ProxyHandler {
    async fn handle_request(
        &mut self,
        _ctx: &HttpContext,
        req: Request<Body>,
    ) -> RequestOrResponse {
        let rules = self.rules.read().await;
        let method = req.method().clone();

        let display_url = if req.uri().scheme().is_some() {
            req.uri().to_string()
        } else {
            let host = req
                .headers()
                .get("host")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("");
            let path = req
                .uri()
                .path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or("/");
            format!("https://{}{}", host, path)
        };

        tracing::debug!(uri = %req.uri(), host = ?req.headers().get("host"), url = %display_url, "incoming request");

        let capturing = traffic::is_capture_enabled();

        let req_headers: Vec<(String, String)> = if capturing {
            req.headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("<binary>").to_string()))
                .collect()
        } else {
            Vec::new()
        };

        for rule in rules.iter().filter(|r| r.enabled) {
            if rule.matches(req.uri(), req.headers(), &method) {
                let (mut parts, body) = req.into_parts();

                // capture original host/scheme before rewriting
                let original_host = parts
                    .headers
                    .get(hyper::header::HOST)
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                let original_scheme = parts.uri.scheme_str().unwrap_or("https").to_string();

                let new_uri = rule.rewrite_uri(&parts.uri);
                parts.uri = new_uri;

                // set forwarding headers so the upstream app knows the original host
                if !original_host.is_empty() {
                    parts.headers.insert(
                        http::header::HeaderName::from_static("x-forwarded-host"),
                        original_host.parse().expect("valid header value"),
                    );
                }
                parts.headers.insert(
                    http::header::HeaderName::from_static("x-forwarded-proto"),
                    original_scheme.parse().expect("valid header value"),
                );

                if rule.preserve_host {
                    parts.extensions.insert(hudsucker::PreserveHost);
                } else {
                    parts.headers.insert(
                        hyper::header::HOST,
                        format!("{}:{}", rule.target.host, rule.target.port)
                            .parse()
                            .expect("valid host:port header value"),
                    );
                }

                tracing::info!(
                    rule = %rule.id,
                    url = %display_url,
                    target = %format!("{}://{}:{}", rule.target.scheme, rule.target.host, rule.target.port),
                    "redirected"
                );

                let _ = self.event_tx.send(ProxyEvent::RequestMatched {
                    rule_id: rule.id.clone(),
                    url: display_url.clone(),
                    method: method.to_string(),
                });

                if capturing {
                    self.pending = Some(PendingTraffic {
                        id: traffic::next_id(),
                        timestamp: chrono::Utc::now().format("%H:%M:%S%.3f").to_string(),
                        method: method.to_string(),
                        url: display_url,
                        rule_id: Some(rule.id.clone()),
                        request_headers: req_headers,
                        started_at: std::time::Instant::now(),
                    });
                }

                return Request::from_parts(parts, body).into();
            }
        }

        let _ = self.event_tx.send(ProxyEvent::RequestPassthrough {
            url: display_url.clone(),
            method: method.to_string(),
        });

        if capturing {
            self.pending = Some(PendingTraffic {
                id: traffic::next_id(),
                timestamp: chrono::Utc::now().format("%H:%M:%S%.3f").to_string(),
                method: method.to_string(),
                url: display_url,
                rule_id: None,
                request_headers: req_headers,
                started_at: std::time::Instant::now(),
            });
        }

        req.into()
    }

    async fn handle_response(&mut self, _ctx: &HttpContext, res: Response<Body>) -> Response<Body> {
        if let Some(pending) = self.pending.take() {
            let response_headers: Vec<(String, String)> = res
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("<binary>").to_string()))
                .collect();

            let record = TrafficRecord {
                id: pending.id,
                timestamp: pending.timestamp,
                method: pending.method,
                url: pending.url,
                status: res.status().as_u16(),
                duration_ms: pending.started_at.elapsed().as_millis() as u64,
                rule_id: pending.rule_id,
                request_headers: pending.request_headers,
                response_headers,
            };

            let _ = self.event_tx.send(ProxyEvent::TrafficEntry(record.clone()));
            self.traffic_buf.write().await.push(record);
        }

        res
    }
}

#[derive(Debug, Clone)]
struct PendingTraffic {
    id: u64,
    timestamp: String,
    method: String,
    url: String,
    rule_id: Option<String>,
    request_headers: Vec<(String, String)>,
    started_at: std::time::Instant,
}
