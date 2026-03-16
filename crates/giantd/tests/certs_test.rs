use giantd::certs::CertAuthority;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

fn tmp() -> TempDir {
    tempfile::tempdir().expect("create temp dir")
}

// -- generation --

#[test]
fn generate_creates_cert_and_key() {
    let dir = tmp();
    let ca = CertAuthority::generate(dir.path()).expect("generate CA");

    assert!(ca.cert_path.exists(), "cert file should exist");
    assert!(ca.key_path.exists(), "key file should exist");
    assert!(
        ca.cert_path.ends_with("ca/giant-proxy-ca.pem"),
        "cert at expected path: {:?}",
        ca.cert_path
    );
    assert!(
        ca.key_path.ends_with("ca/giant-proxy-ca-key.pem"),
        "key at expected path: {:?}",
        ca.key_path
    );
}

#[test]
fn generated_cert_is_valid_pem() {
    let dir = tmp();
    let ca = CertAuthority::generate(dir.path()).unwrap();

    let cert_pem = std::fs::read_to_string(&ca.cert_path).unwrap();
    assert!(
        cert_pem.starts_with("-----BEGIN CERTIFICATE-----"),
        "cert should be PEM format"
    );
    assert!(cert_pem.contains("-----END CERTIFICATE-----"));
}

#[test]
fn generated_key_is_valid_pem() {
    let dir = tmp();
    let ca = CertAuthority::generate(dir.path()).unwrap();

    let key_pem = std::fs::read_to_string(&ca.key_path).unwrap();
    assert!(
        key_pem.contains("PRIVATE KEY"),
        "key should be PEM format, got: {}",
        &key_pem[..50]
    );
}

#[test]
fn generated_key_has_600_permissions() {
    let dir = tmp();
    let ca = CertAuthority::generate(dir.path()).unwrap();

    let meta = std::fs::metadata(&ca.key_path).unwrap();
    let mode = meta.permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o600,
        "key should have 600 permissions, got {:o}",
        mode
    );
}

#[test]
fn generated_cert_loadable_by_hudsucker() {
    // the daemon uses hudsucker::rcgen to parse the cert -- make sure it works
    let dir = tmp();
    let ca = CertAuthority::generate(dir.path()).unwrap();

    let key_pem = std::fs::read_to_string(&ca.key_path).unwrap();
    let cert_pem = std::fs::read_to_string(&ca.cert_path).unwrap();

    let key_pair = hudsucker::rcgen::KeyPair::from_pem(&key_pem)
        .expect("hudsucker should parse generated key");
    let _issuer = hudsucker::rcgen::Issuer::from_ca_cert_pem(&cert_pem, key_pair)
        .expect("hudsucker should create issuer from generated cert");
}

// -- load --

#[test]
fn load_after_generate_succeeds() {
    let dir = tmp();
    CertAuthority::generate(dir.path()).unwrap();

    let loaded = CertAuthority::load(dir.path());
    assert!(loaded.is_ok(), "load should succeed after generate");
    let ca = loaded.unwrap();
    assert!(ca.cert_path.exists());
    assert!(ca.key_path.exists());
}

#[test]
fn load_without_generate_fails() {
    let dir = tmp();
    let result = CertAuthority::load(dir.path());
    assert!(result.is_err(), "load should fail when no cert exists");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("not found"),
        "error should mention not found: {}",
        err
    );
}

#[test]
fn load_with_missing_key_fails() {
    let dir = tmp();
    let ca = CertAuthority::generate(dir.path()).unwrap();
    std::fs::remove_file(&ca.key_path).unwrap();

    let result = CertAuthority::load(dir.path());
    assert!(result.is_err(), "load should fail when key is missing");
}

#[test]
fn load_with_missing_cert_fails() {
    let dir = tmp();
    let ca = CertAuthority::generate(dir.path()).unwrap();
    std::fs::remove_file(&ca.cert_path).unwrap();

    let result = CertAuthority::load(dir.path());
    assert!(result.is_err(), "load should fail when cert is missing");
}

// -- permissions check --

#[test]
fn check_permissions_passes_for_600() {
    let dir = tmp();
    let ca = CertAuthority::generate(dir.path()).unwrap();
    assert!(ca.check_permissions().is_ok());
}

#[test]
fn check_permissions_fails_for_644() {
    let dir = tmp();
    let ca = CertAuthority::generate(dir.path()).unwrap();

    // loosen permissions
    std::fs::set_permissions(&ca.key_path, std::fs::Permissions::from_mode(0o644)).unwrap();

    let result = ca.check_permissions();
    assert!(result.is_err(), "should reject 644 permissions");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("644"),
        "error should mention actual mode: {}",
        err
    );
}

#[test]
fn check_permissions_fails_for_777() {
    let dir = tmp();
    let ca = CertAuthority::generate(dir.path()).unwrap();

    std::fs::set_permissions(&ca.key_path, std::fs::Permissions::from_mode(0o777)).unwrap();

    assert!(ca.check_permissions().is_err());
}

// -- generate is idempotent (overwrites) --

#[test]
fn generate_twice_overwrites() {
    let dir = tmp();
    let ca1 = CertAuthority::generate(dir.path()).unwrap();
    let cert1 = std::fs::read_to_string(&ca1.cert_path).unwrap();

    let ca2 = CertAuthority::generate(dir.path()).unwrap();
    let cert2 = std::fs::read_to_string(&ca2.cert_path).unwrap();

    // new keypair each time
    assert_ne!(cert1, cert2, "regenerate should produce a new cert");
}
