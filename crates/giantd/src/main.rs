use giantd::api::{self, AppState};
use giantd::config;
use giantd::events::EventBus;
use giantd::pac;
use giantd::pid;
use giantd::proxy::ProxyHandler;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let foreground = std::env::args().any(|a| a == "--foreground");
    let port_override: Option<u16> = std::env::args()
        .position(|a| a == "--port")
        .and_then(|i| std::env::args().nth(i + 1))
        .and_then(|p| p.parse().ok());

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("giantd=info".parse()?))
        .init();

    let config = config::load_config()?;
    config::migrate_config()?;
    let listen_port = port_override.unwrap_or(config.listen_port);
    let pac_port = config.pac_port;
    let config_dir = config::config_dir();

    std::fs::create_dir_all(&config_dir)?;
    pid::write_pid(&config_dir)?;

    let event_bus = Arc::new(EventBus::new(256));
    let rules: Arc<RwLock<Vec<giantd::rules::Rule>>> = Arc::new(RwLock::new(Vec::new()));

    let state = AppState {
        config: Arc::new(RwLock::new(config)),
        rules: rules.clone(),
        active_profile: Arc::new(RwLock::new(None)),
        event_bus: event_bus.clone(),
        started_at: Arc::new(RwLock::new(Some(chrono::Utc::now()))),
    };

    // remove stale socket
    let socket_path = config_dir.join("giantd.sock");
    let _ = std::fs::remove_file(&socket_path);

    // control API on unix socket
    let api_router = api::routes(state.clone());
    let api_listener = tokio::net::UnixListener::bind(&socket_path)?;
    let api_task = tokio::spawn(async move {
        axum::serve(api_listener, api_router).await.ok();
    });

    // pac server on tcp
    let pac_state = state.clone();
    let pac_task = tokio::spawn(async move {
        let pac_router = axum::Router::new()
            .route(
                "/proxy.pac",
                axum::routing::get(
                    move |axum::extract::State(s): axum::extract::State<
                        Arc<RwLock<Vec<giantd::rules::Rule>>>,
                    >| async move {
                        let rules = s.read().await;
                        let pac_content = pac::generate_pac(&rules, listen_port);
                        (
                            [(
                                axum::http::header::CONTENT_TYPE,
                                "application/x-ns-proxy-autoconfig",
                            )],
                            pac_content,
                        )
                    },
                ),
            )
            .with_state(pac_state.rules.clone());

        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", pac_port))
            .await
            .expect("failed to bind PAC server");
        tracing::info!("pac server listening on 127.0.0.1:{}", pac_port);
        axum::serve(listener, pac_router).await.ok();
    });

    // proxy listener via hudsucker
    let proxy_handler = ProxyHandler {
        rules: rules.clone(),
        event_tx: event_bus.sender(),
    };

    let proxy_task = tokio::spawn(async move {
        let ca = match giantd::certs::CertAuthority::load(&config_dir) {
            Ok(ca) => ca,
            Err(e) => {
                tracing::warn!("CA not loaded, proxy will not intercept HTTPS: {}", e);
                return;
            }
        };

        let key_pem = std::fs::read_to_string(&ca.key_path).expect("read CA key");
        let cert_pem = std::fs::read_to_string(&ca.cert_path).expect("read CA cert");

        let key_pair = match hudsucker::rcgen::KeyPair::from_pem(&key_pem) {
            Ok(kp) => kp,
            Err(e) => {
                tracing::error!("failed to parse CA key: {}", e);
                return;
            }
        };

        let issuer = match hudsucker::rcgen::Issuer::from_ca_cert_pem(&cert_pem, key_pair) {
            Ok(i) => i,
            Err(e) => {
                tracing::error!("failed to create issuer: {}", e);
                return;
            }
        };

        let authority = hudsucker::certificate_authority::RcgenAuthority::new(
            issuer,
            1000,
            hudsucker::rustls::crypto::aws_lc_rs::default_provider(),
        );

        match hudsucker::Proxy::builder()
            .with_addr(std::net::SocketAddr::from(([127, 0, 0, 1], listen_port)))
            .with_ca(authority)
            .with_rustls_connector(hudsucker::rustls::crypto::aws_lc_rs::default_provider())
            .with_http_handler(proxy_handler)
            .build()
        {
            Ok(proxy) => {
                tracing::info!("proxy listening on 127.0.0.1:{}", listen_port);
                if let Err(e) = proxy.start().await {
                    tracing::error!("proxy error: {}", e);
                }
            }
            Err(e) => {
                tracing::error!("failed to build proxy: {}", e);
            }
        }
    });

    if foreground {
        tracing::info!("running in foreground mode");
    }

    tracing::info!("giantd started (pid {})", std::process::id());

    // signal handling
    let shutdown_config_dir = config::config_dir();
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("received shutdown signal");
        }
        _ = api_task => {}
        _ = pac_task => {}
        _ = proxy_task => {}
    }

    // cleanup
    tracing::info!("shutting down");
    let _ = std::fs::remove_file(shutdown_config_dir.join("giantd.sock"));
    pid::cleanup_pid(&shutdown_config_dir).ok();
    let _ = config::write_state(&giantd::config::DaemonState {
        running: false,
        active_profile: None,
        also_profiles: vec![],
        listen_addr: format!("127.0.0.1:{}", listen_port),
        routing_mode: "manual".to_string(),
        pid: std::process::id(),
        started_at: String::new(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        rules: vec![],
        proxy_services: vec![],
    });

    Ok(())
}
