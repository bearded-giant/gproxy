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
        if !self.socket_path.exists() {
            return false;
        }
        std::os::unix::net::UnixStream::connect(&self.socket_path).is_ok()
    }

    pub fn cleanup_stale(&self) {
        if self.socket_path.exists() && !self.is_daemon_running() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }

    pub fn find_giantd() -> PathBuf {
        // tauri sidecar: next to our own binary (Contents/MacOS/ in .app bundle)
        if let Some(sibling) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("giantd")))
            .filter(|p| p.exists())
        {
            return sibling;
        }

        // cargo install location
        if let Some(cargo) = dirs::home_dir().map(|h| h.join(".cargo/bin/giantd")) {
            if cargo.exists() {
                return cargo;
            }
        }

        // homebrew
        for brew_dir in &["/opt/homebrew/bin/giantd", "/usr/local/bin/giantd"] {
            let p = PathBuf::from(brew_dir);
            if p.exists() {
                return p;
            }
        }

        PathBuf::from("giantd")
    }

    pub async fn ensure_daemon_started(&self) -> Result<(), String> {
        self.cleanup_stale();
        if self.is_daemon_running() {
            return Ok(());
        }

        let giantd_path = Self::find_giantd();
        tracing::info!("starting daemon from: {:?}", giantd_path);

        std::process::Command::new(&giantd_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to start daemon (path: {:?}): {}", giantd_path, e))?;

        for _ in 0..20 {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            if self.is_daemon_running() {
                return Ok(());
            }
        }
        Err("daemon spawned but socket never appeared".to_string())
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

        let req = match &body {
            Some(b) => {
                let body_bytes = serde_json::to_vec(b)?;
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Full::new(Bytes::from(body_bytes)))?
            }
            None => Request::builder()
                .method(method)
                .uri(uri)
                .body(Full::new(Bytes::new()))?,
        };

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
