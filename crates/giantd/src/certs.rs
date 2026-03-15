use crate::errors::{GiantError, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub struct CertAuthority {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

impl CertAuthority {
    pub fn generate(config_dir: &Path) -> Result<Self> {
        let ca_dir = config_dir.join("ca");
        std::fs::create_dir_all(&ca_dir)?;

        let mut params = rcgen::CertificateParams::default();
        params.distinguished_name = rcgen::DistinguishedName::new();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "Giant Proxy CA");
        params
            .distinguished_name
            .push(rcgen::DnType::OrganizationName, "Giant Proxy");
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        params.not_after = time::OffsetDateTime::now_utc() + time::Duration::days(3650);

        let key_pair = rcgen::KeyPair::generate()
            .map_err(|e| GiantError::CertError(format!("failed to generate key pair: {}", e)))?;

        let cert = params
            .self_signed(&key_pair)
            .map_err(|e| GiantError::CertError(format!("failed to generate CA cert: {}", e)))?;

        let cert_path = ca_dir.join("giant-proxy-ca.pem");
        let key_path = ca_dir.join("giant-proxy-ca-key.pem");

        std::fs::write(&cert_path, cert.pem())?;

        std::fs::write(&key_path, key_pair.serialize_pem())?;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))?;

        Ok(Self {
            cert_path,
            key_path,
        })
    }

    pub fn load(config_dir: &Path) -> Result<Self> {
        let ca_dir = config_dir.join("ca");
        let cert_path = ca_dir.join("giant-proxy-ca.pem");
        let key_path = ca_dir.join("giant-proxy-ca-key.pem");

        if !cert_path.exists() || !key_path.exists() {
            return Err(GiantError::CertError(
                "CA cert or key not found. Run `giant-proxy init` first.".to_string(),
            ));
        }

        Ok(Self {
            cert_path,
            key_path,
        })
    }

    pub fn install_trust_store(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            let status = std::process::Command::new("sudo")
                .args([
                    "security",
                    "add-trusted-cert",
                    "-d",
                    "-r",
                    "trustRoot",
                    "-k",
                    "/Library/Keychains/System.keychain",
                    self.cert_path.to_str().unwrap(),
                ])
                .status()?;

            if !status.success() {
                return Err(GiantError::CertError(
                    "failed to install CA cert to system keychain".to_string(),
                ));
            }
        }

        #[cfg(target_os = "linux")]
        {
            let dest = "/usr/local/share/ca-certificates/giant-proxy-ca.crt";
            let cp = std::process::Command::new("sudo")
                .args(["cp", self.cert_path.to_str().unwrap(), dest])
                .status()?;
            if !cp.success() {
                return Err(GiantError::CertError(format!(
                    "failed to copy CA cert to {}",
                    dest
                )));
            }

            let update = std::process::Command::new("sudo")
                .args(["update-ca-certificates"])
                .status()?;
            if !update.success() {
                return Err(GiantError::CertError(
                    "update-ca-certificates failed".to_string(),
                ));
            }
        }

        Ok(())
    }

    pub fn is_installed(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            return std::process::Command::new("security")
                .args([
                    "find-certificate",
                    "-c",
                    "Giant Proxy CA",
                    "/Library/Keychains/System.keychain",
                ])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
        }
        #[cfg(target_os = "linux")]
        {
            return std::path::Path::new("/usr/local/share/ca-certificates/giant-proxy-ca.crt")
                .exists();
        }
        #[allow(unreachable_code)]
        false
    }

    pub fn check_permissions(&self) -> Result<()> {
        let metadata = std::fs::metadata(&self.key_path)?;
        let permissions = metadata.permissions();
        let mode = permissions.mode() & 0o777;
        if mode != 0o600 {
            return Err(GiantError::CertError(format!(
                "CA key has permissions {:o}, expected 600",
                mode
            )));
        }
        Ok(())
    }
}
