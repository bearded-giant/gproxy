use crate::events::ProxyEvent;
use crate::rules::Rule;
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
}

impl HttpHandler for ProxyHandler {
    async fn handle_request(
        &mut self,
        _ctx: &HttpContext,
        req: Request<Body>,
    ) -> RequestOrResponse {
        let rules = self.rules.read().await;
        let method = req.method().clone();

        // reconstruct full url for logging
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

        for rule in rules.iter().filter(|r| r.enabled) {
            if rule.matches(req.uri(), req.headers(), &method) {
                let (mut parts, body) = req.into_parts();
                let new_uri = rule.rewrite_uri(&parts.uri);
                parts.uri = new_uri;

                if !rule.preserve_host {
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
                    url: display_url,
                    method: method.to_string(),
                });

                return Request::from_parts(parts, body).into();
            }
        }

        let _ = self.event_tx.send(ProxyEvent::RequestPassthrough {
            url: display_url,
            method: method.to_string(),
        });

        req.into()
    }

    async fn handle_response(&mut self, _ctx: &HttpContext, res: Response<Body>) -> Response<Body> {
        res
    }
}
