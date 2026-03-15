use std::path::PathBuf;
use tokio::net::UnixStream;
use tokio_tungstenite::WebSocketStream;

pub struct DaemonClient {
    socket_path: PathBuf,
}

impl DaemonClient {
    pub fn new() -> Self {
        let socket_path = dirs::home_dir()
            .expect("home directory must exist")
            .join(".giant-proxy")
            .join("giantd.sock");
        Self { socket_path }
    }

    pub async fn get(
        &self,
        path: &str,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        self.request("GET", path, None).await
    }

    pub async fn post(
        &self,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        self.request("POST", path, body).await
    }

    #[allow(dead_code)]
    pub async fn put(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        self.request("PUT", path, Some(body)).await
    }

    #[allow(dead_code)]
    pub async fn delete(
        &self,
        path: &str,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        self.request("DELETE", path, None).await
    }

    pub fn is_daemon_running(&self) -> bool {
        self.socket_path.exists()
    }

    pub async fn connect_events(
        &self,
    ) -> Result<WebSocketStream<UnixStream>, Box<dyn std::error::Error + Send + Sync>> {
        let stream = UnixStream::connect(&self.socket_path).await?;
        let url = "ws://localhost/events";
        let (ws_stream, _) = tokio_tungstenite::client_async(url, stream).await?;
        Ok(ws_stream)
    }

    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        use http_body_util::{BodyExt, Full};
        use hyper::body::Bytes;
        use hyper::Request;
        use hyperlocal::{UnixClientExt, Uri};

        let uri = Uri::new(&self.socket_path, path);

        let body_bytes = match &body {
            Some(b) => serde_json::to_vec(b)?,
            None => vec![],
        };

        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from(body_bytes)))?;

        let client = hyper_util::client::legacy::Client::unix();
        let resp = client.request(req).await?;
        let body = resp.into_body().collect().await?.to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&body)?;
        Ok(value)
    }
}

impl Default for DaemonClient {
    fn default() -> Self {
        Self::new()
    }
}
