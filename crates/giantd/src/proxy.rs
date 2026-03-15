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
                    target = %format!("{}://{}:{}", rule.target.scheme, rule.target.host, rule.target.port),
                    "redirected"
                );

                let _ = self.event_tx.send(ProxyEvent::RequestMatched {
                    rule_id: rule.id.clone(),
                    url: parts.uri.to_string(),
                });

                return Request::from_parts(parts, body).into();
            }
        }

        req.into()
    }

    async fn handle_response(&mut self, _ctx: &HttpContext, res: Response<Body>) -> Response<Body> {
        res
    }
}
