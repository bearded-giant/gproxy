use hyper::Uri;
use hyper_util::client::legacy::connect::HttpConnector;
use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use tower::Service;

pub type RouteOverrides = Arc<RwLock<HashMap<String, SocketAddr>>>;

#[derive(Clone)]
pub struct RoutingConnector {
    inner: HttpConnector,
    overrides: RouteOverrides,
}

impl RoutingConnector {
    pub fn new(overrides: RouteOverrides) -> Self {
        Self {
            inner: HttpConnector::new(),
            overrides,
        }
    }
}

impl Service<Uri> for RoutingConnector {
    type Response = <HttpConnector as Service<Uri>>::Response;
    type Error = <HttpConnector as Service<Uri>>::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, uri: Uri) -> Self::Future {
        let authority = uri.authority().map(|a| a.to_string()).unwrap_or_default();

        let target = self
            .overrides
            .read()
            .ok()
            .and_then(|m| m.get(&authority).copied());

        if let Some(addr) = target {
            let rerouted = Uri::builder()
                .scheme("http")
                .authority(format!("{}:{}", addr.ip(), addr.port()))
                .path_and_query(
                    uri.path_and_query()
                        .map(|pq| pq.as_str())
                        .unwrap_or("/"),
                )
                .build()
                .unwrap_or(uri);
            tracing::debug!(original = %authority, target = %addr, "routing override");
            Box::pin(self.inner.call(rerouted))
        } else {
            Box::pin(self.inner.call(uri))
        }
    }
}
