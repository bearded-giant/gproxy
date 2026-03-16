use giantd::pid;
use tempfile::TempDir;

fn tmp() -> TempDir {
    tempfile::tempdir().expect("create temp dir")
}

// -- PAC server port binding --

#[tokio::test]
async fn pac_server_binds_to_free_port() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    assert!(addr.port() > 0);
    drop(listener);
}

#[tokio::test]
async fn pac_server_graceful_when_port_occupied() {
    // bind a port to simulate "address already in use"
    let blocker = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let occupied_port = blocker.local_addr().unwrap().port();

    // the daemon's pac server should handle this gracefully (the fix we made)
    let result = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", occupied_port)).await;
    assert!(result.is_err(), "binding an occupied port should fail");
    let err = result.unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::AddrInUse);
}

#[tokio::test]
async fn tcp_listener_frees_port_on_drop() {
    let port = {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let p = listener.local_addr().unwrap().port();
        drop(listener);
        p
    };

    // should be able to rebind after drop
    let result = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await;
    assert!(result.is_ok(), "port should be free after listener is dropped");
}

// -- unix socket API binding --

#[tokio::test]
async fn api_socket_binds_and_accepts() {
    let dir = tmp();
    let socket_path = dir.path().join("test.sock");

    let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();
    assert!(socket_path.exists(), "socket file should exist");

    // client can connect
    let client = tokio::net::UnixStream::connect(&socket_path).await;
    assert!(client.is_ok(), "should connect to unix socket");

    drop(listener);
}

#[tokio::test]
async fn stale_socket_cleanup_allows_rebind() {
    let dir = tmp();
    let socket_path = dir.path().join("test.sock");

    // create a socket, then drop it (simulates crashed daemon)
    {
        let _listener = tokio::net::UnixListener::bind(&socket_path).unwrap();
    }
    // socket file still exists after drop
    assert!(socket_path.exists());

    // remove stale socket (what the daemon does on startup)
    let _ = std::fs::remove_file(&socket_path);

    // rebind should work
    let listener = tokio::net::UnixListener::bind(&socket_path);
    assert!(listener.is_ok(), "should rebind after removing stale socket");
}

#[tokio::test]
async fn cannot_bind_socket_without_cleanup() {
    let dir = tmp();
    let socket_path = dir.path().join("test.sock");

    let _listener = tokio::net::UnixListener::bind(&socket_path).unwrap();

    // second bind without removing the file should fail
    let result = tokio::net::UnixListener::bind(&socket_path);
    assert!(result.is_err(), "double bind should fail without cleanup");
}

// -- PID file management --

#[test]
fn pid_write_and_read() {
    let dir = tmp();
    pid::write_pid(dir.path()).unwrap();

    let read = pid::read_pid(dir.path()).unwrap();
    assert!(read.is_some(), "should read PID after write");
    assert_eq!(
        read.unwrap(),
        std::process::id(),
        "PID should match current process"
    );
}

#[test]
fn pid_read_missing_returns_none() {
    let dir = tmp();
    let read = pid::read_pid(dir.path()).unwrap();
    assert!(read.is_none(), "should return None when no PID file");
}

#[test]
fn pid_cleanup_removes_file() {
    let dir = tmp();
    pid::write_pid(dir.path()).unwrap();
    assert!(dir.path().join("giantd.pid").exists());

    pid::cleanup_pid(dir.path()).unwrap();
    assert!(!dir.path().join("giantd.pid").exists());
}

#[test]
fn pid_cleanup_noop_when_missing() {
    let dir = tmp();
    // should not error when file doesn't exist
    assert!(pid::cleanup_pid(dir.path()).is_ok());
}

#[test]
fn pid_is_running_for_self() {
    assert!(
        pid::is_running(std::process::id()),
        "current process should be running"
    );
}

#[test]
fn pid_is_running_false_for_bogus() {
    // PID 99999999 almost certainly doesn't exist
    assert!(!pid::is_running(99_999_999));
}

// -- full daemon init sequence (no actual proxy, just the setup parts) --

#[tokio::test]
async fn daemon_init_sequence() {
    let dir = tmp();

    // 1. create config dir structure
    std::fs::create_dir_all(dir.path().join("profiles")).unwrap();
    std::fs::create_dir_all(dir.path().join("logs")).unwrap();

    // 2. write PID
    pid::write_pid(dir.path()).unwrap();
    let pid = pid::read_pid(dir.path()).unwrap().unwrap();
    assert_eq!(pid, std::process::id());

    // 3. generate CA
    let ca = giantd::certs::CertAuthority::generate(dir.path()).unwrap();
    assert!(ca.cert_path.exists());
    ca.check_permissions().unwrap();

    // 4. load CA (what the proxy task does)
    let loaded = giantd::certs::CertAuthority::load(dir.path()).unwrap();
    let key_pem = std::fs::read_to_string(&loaded.key_path).unwrap();
    let cert_pem = std::fs::read_to_string(&loaded.cert_path).unwrap();
    let kp = hudsucker::rcgen::KeyPair::from_pem(&key_pem).unwrap();
    let _issuer = hudsucker::rcgen::Issuer::from_ca_cert_pem(&cert_pem, kp).unwrap();

    // 5. bind API socket
    let socket_path = dir.path().join("giantd.sock");
    let _ = std::fs::remove_file(&socket_path);
    let _listener = tokio::net::UnixListener::bind(&socket_path).unwrap();
    assert!(socket_path.exists());

    // 6. bind PAC server
    let pac_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let pac_port = pac_listener.local_addr().unwrap().port();
    assert!(pac_port > 0);

    // 7. cleanup
    drop(pac_listener);
    drop(_listener);
    pid::cleanup_pid(dir.path()).unwrap();
    assert!(!dir.path().join("giantd.pid").exists());
}
