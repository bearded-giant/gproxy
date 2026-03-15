use crate::errors::Result;
use std::path::Path;

#[cfg(target_os = "macos")]
pub fn set_system_proxy(port: u16) -> Result<Vec<String>> {
    let output = std::process::Command::new("networksetup")
        .args(["-listallnetworkservices"])
        .output()?;
    let services_str = String::from_utf8_lossy(&output.stdout);
    let mut modified = Vec::new();

    for line in services_str.lines().skip(1) {
        let service = line.trim();
        if service.starts_with('*') || service.is_empty() {
            continue;
        }

        let info = std::process::Command::new("networksetup")
            .args(["-getinfo", service])
            .output()?;
        let info_str = String::from_utf8_lossy(&info.stdout);
        let has_ip = info_str
            .lines()
            .any(|l| l.starts_with("IP address") && !l.contains("none") && l.contains('.'));

        if has_ip {
            let port_str = port.to_string();
            std::process::Command::new("networksetup")
                .args(["-setsecurewebproxy", service, "127.0.0.1", &port_str])
                .status()?;
            std::process::Command::new("networksetup")
                .args(["-setwebproxy", service, "127.0.0.1", &port_str])
                .status()?;
            modified.push(service.to_string());
        }
    }

    Ok(modified)
}

#[cfg(target_os = "linux")]
pub fn set_system_proxy(port: u16) -> Result<Vec<String>> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let port_str = port.to_string();
    let mut modified = Vec::new();

    if desktop.contains("GNOME") || desktop.contains("Unity") {
        std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy", "mode", "manual"])
            .status()?;
        std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.http", "host", "127.0.0.1"])
            .status()?;
        std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.http", "port", &port_str])
            .status()?;
        std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.https", "host", "127.0.0.1"])
            .status()?;
        std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy.https", "port", &port_str])
            .status()?;
        modified.push("gnome".to_string());
    } else if desktop.contains("KDE") {
        let kwrite = ["kwriteconfig6", "kwriteconfig5"]
            .iter()
            .find(|cmd| which(cmd))
            .copied();

        if let Some(cmd) = kwrite {
            let proxy = format!("http://127.0.0.1:{}", port);
            std::process::Command::new(cmd)
                .args([
                    "--file",
                    "kioslaverc",
                    "--group",
                    "Proxy Settings",
                    "--key",
                    "ProxyType",
                    "1",
                ])
                .status()?;
            std::process::Command::new(cmd)
                .args([
                    "--file",
                    "kioslaverc",
                    "--group",
                    "Proxy Settings",
                    "--key",
                    "httpProxy",
                    &proxy,
                ])
                .status()?;
            std::process::Command::new(cmd)
                .args([
                    "--file",
                    "kioslaverc",
                    "--group",
                    "Proxy Settings",
                    "--key",
                    "httpsProxy",
                    &proxy,
                ])
                .status()?;
            modified.push("kde".to_string());
        }
    }

    Ok(modified)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn set_system_proxy(_port: u16) -> Result<Vec<String>> {
    Ok(vec![])
}

#[cfg(target_os = "macos")]
pub fn clear_system_proxy(services: &[String]) -> Result<()> {
    for service in services {
        std::process::Command::new("networksetup")
            .args(["-setsecurewebproxystate", service, "off"])
            .status()?;
        std::process::Command::new("networksetup")
            .args(["-setwebproxystate", service, "off"])
            .status()?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn clear_system_proxy(services: &[String]) -> Result<()> {
    for service in services {
        match service.as_str() {
            "gnome" => {
                std::process::Command::new("gsettings")
                    .args(["set", "org.gnome.system.proxy", "mode", "none"])
                    .status()?;
            }
            "kde" => {
                let kwrite = ["kwriteconfig6", "kwriteconfig5"]
                    .iter()
                    .find(|cmd| which(cmd))
                    .copied();
                if let Some(cmd) = kwrite {
                    std::process::Command::new(cmd)
                        .args([
                            "--file",
                            "kioslaverc",
                            "--group",
                            "Proxy Settings",
                            "--key",
                            "ProxyType",
                            "0",
                        ])
                        .status()?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn clear_system_proxy(_services: &[String]) -> Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn set_pac_proxy(pac_url: &str) -> Result<Vec<String>> {
    let output = std::process::Command::new("networksetup")
        .args(["-listallnetworkservices"])
        .output()?;
    let services_str = String::from_utf8_lossy(&output.stdout);
    let mut modified = Vec::new();

    for line in services_str.lines().skip(1) {
        let service = line.trim();
        if service.starts_with('*') || service.is_empty() {
            continue;
        }

        let info = std::process::Command::new("networksetup")
            .args(["-getinfo", service])
            .output()?;
        let info_str = String::from_utf8_lossy(&info.stdout);
        let has_ip = info_str
            .lines()
            .any(|l| l.starts_with("IP address") && !l.contains("none") && l.contains('.'));

        if has_ip {
            std::process::Command::new("networksetup")
                .args(["-setautoproxyurl", service, pac_url])
                .status()?;
            modified.push(service.to_string());
        }
    }

    Ok(modified)
}

#[cfg(target_os = "linux")]
pub fn set_pac_proxy(pac_url: &str) -> Result<Vec<String>> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let mut modified = Vec::new();

    if desktop.contains("GNOME") || desktop.contains("Unity") {
        std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy", "mode", "auto"])
            .status()?;
        std::process::Command::new("gsettings")
            .args(["set", "org.gnome.system.proxy", "autoconfig-url", pac_url])
            .status()?;
        modified.push("gnome".to_string());
    }

    Ok(modified)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn set_pac_proxy(_pac_url: &str) -> Result<Vec<String>> {
    Ok(vec![])
}

#[cfg(target_os = "macos")]
pub fn clear_pac_proxy(services: &[String]) -> Result<()> {
    for service in services {
        std::process::Command::new("networksetup")
            .args(["-setautoproxystate", service, "off"])
            .status()?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn clear_pac_proxy(services: &[String]) -> Result<()> {
    for service in services {
        if service == "gnome" {
            std::process::Command::new("gsettings")
                .args(["set", "org.gnome.system.proxy", "mode", "none"])
                .status()?;
        }
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn clear_pac_proxy(_services: &[String]) -> Result<()> {
    Ok(())
}

pub fn generate_env_snippet(port: u16, ca_path: &Path, bypass: &[String]) -> String {
    let no_proxy = if bypass.is_empty() {
        "localhost,127.0.0.1".to_string()
    } else {
        bypass.join(",")
    };
    format!(
        "export HTTP_PROXY=http://127.0.0.1:{port}\n\
         export HTTPS_PROXY=http://127.0.0.1:{port}\n\
         export NODE_EXTRA_CA_CERTS={ca_path}\n\
         export NO_PROXY={no_proxy}",
        port = port,
        ca_path = ca_path.display(),
        no_proxy = no_proxy,
    )
}

#[cfg(target_os = "linux")]
fn which(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
