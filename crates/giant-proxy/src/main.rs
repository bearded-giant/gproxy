mod client;

use clap::{Parser, Subcommand};
use client::DaemonClient;

const LAUNCHD_PLIST: &str = include_str!("../../../service/com.giantproxy.daemon.plist");
const SYSTEMD_UNIT: &str = include_str!("../../../service/giantd.service");

#[derive(Parser)]
#[command(name = "giant-proxy", about = "HTTPS proxy with Map Remote rules")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum DaemonAction {
    Start,
    Stop,
    Install,
    Uninstall,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    On {
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        system_proxy: bool,
        #[arg(long)]
        pac: bool,
    },
    Off,
    Status,
    Use {
        profile: String,
        #[arg(long)]
        also: Option<Vec<String>>,
        #[arg(long)]
        rule: Option<String>,
    },
    Toggle {
        rule_id: String,
    },
    Profiles,
    Doctor {
        #[arg(long)]
        fix: bool,
    },
    Env,
    Import {
        file: String,
        #[arg(long, value_name = "NAME")]
        r#as: Option<String>,
        #[arg(long)]
        all: bool,
    },
    Export {
        profile: String,
        #[arg(long, default_value = "toml")]
        format: String,
    },
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    Uninstall,
    Version,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let client = DaemonClient::new();

    match cli.command {
        Commands::Init => {
            println!("initializing giant-proxy...");
            let config_dir = dirs::home_dir()
                .expect("home directory must exist")
                .join(".giant-proxy");
            std::fs::create_dir_all(&config_dir).expect("failed to create config dir");
            std::fs::create_dir_all(config_dir.join("profiles"))
                .expect("failed to create profiles dir");
            std::fs::create_dir_all(config_dir.join("logs")).expect("failed to create logs dir");
            println!("config directory: {}", config_dir.display());
            println!("run `giant-proxy on` to start the proxy");
        }
        Commands::On { profile, .. } => {
            ensure_daemon(&client).await;
            let profile_name = profile.unwrap_or_else(|| "preprod".to_string());
            match client.post(&format!("/use/{}", profile_name), None).await {
                Ok(resp) => println!("proxy on: {}", resp),
                Err(e) => eprintln!("error: {}", e),
            }
        }
        Commands::Off => {
            if client.is_daemon_running() {
                match client.post("/stop", None).await {
                    Ok(resp) => println!("proxy off: {}", resp),
                    Err(e) => eprintln!("error: {}", e),
                }
            } else {
                println!("daemon not running");
            }
        }
        Commands::Status => {
            if !client.is_daemon_running() {
                println!("daemon not running");
                return;
            }
            match client.get("/status").await {
                Ok(resp) => println!("{}", serde_json::to_string_pretty(&resp).unwrap()),
                Err(e) => eprintln!("error: {}", e),
            }
        }
        Commands::Use { profile, .. } => {
            ensure_daemon(&client).await;
            match client.post(&format!("/use/{}", profile), None).await {
                Ok(resp) => println!("switched to {}: {}", profile, resp),
                Err(e) => eprintln!("error: {}", e),
            }
        }
        Commands::Toggle { rule_id } => {
            ensure_daemon(&client).await;
            match client
                .post(&format!("/rules/{}/toggle", rule_id), None)
                .await
            {
                Ok(resp) => println!("toggled: {}", resp),
                Err(e) => eprintln!("error: {}", e),
            }
        }
        Commands::Profiles => {
            ensure_daemon(&client).await;
            match client.get("/profiles").await {
                Ok(resp) => println!("{}", serde_json::to_string_pretty(&resp).unwrap()),
                Err(e) => eprintln!("error: {}", e),
            }
        }
        Commands::Doctor { .. } => {
            println!("running diagnostics...");
            let config_dir = dirs::home_dir()
                .expect("home directory must exist")
                .join(".giant-proxy");

            let ca_cert = config_dir.join("ca").join("giant-proxy-ca.pem");
            println!(
                "  CA cert: {}",
                if ca_cert.exists() { "found" } else { "MISSING" }
            );

            let ca_key = config_dir.join("ca").join("giant-proxy-ca-key.pem");
            println!(
                "  CA key:  {}",
                if ca_key.exists() { "found" } else { "MISSING" }
            );

            println!(
                "  daemon:  {}",
                if client.is_daemon_running() {
                    "running"
                } else {
                    "stopped"
                }
            );
        }
        Commands::Env => {
            if !client.is_daemon_running() {
                let config_dir = dirs::home_dir()
                    .expect("home directory must exist")
                    .join(".giant-proxy");
                let ca_path = config_dir.join("ca").join("giant-proxy-ca.pem");
                println!("export HTTP_PROXY=http://127.0.0.1:8080");
                println!("export HTTPS_PROXY=http://127.0.0.1:8080");
                println!("export NODE_EXTRA_CA_CERTS={}", ca_path.display());
                println!("export NO_PROXY=localhost,127.0.0.1");
            } else {
                match client.get("/env").await {
                    Ok(resp) => {
                        if let Some(snippet) = resp.get("shell_snippet").and_then(|s| s.as_str()) {
                            println!("{}", snippet);
                        }
                    }
                    Err(e) => eprintln!("error: {}", e),
                }
            }
        }
        Commands::Import {
            file,
            r#as: name,
            all,
        } => {
            let path = std::path::Path::new(&file);
            if !path.exists() {
                eprintln!("file not found: {}", file);
                std::process::exit(1);
            }

            if all {
                match giantd::convert::import_legacy(path) {
                    Ok(profiles) => {
                        for (pname, profile) in &profiles {
                            match giantd::convert::save_profile(profile) {
                                Ok(()) => println!("imported profile: {}", pname),
                                Err(e) => eprintln!("failed to import {}: {}", pname, e),
                            }
                        }
                        println!("imported {} profiles", profiles.len());
                    }
                    Err(e) => {
                        eprintln!("import failed: {}", e);
                        std::process::exit(1);
                    }
                }
            } else if let Some(profile_name) = name {
                match giantd::convert::import_legacy_profile(path, &profile_name) {
                    Ok(profile) => match giantd::convert::save_profile(&profile) {
                        Ok(()) => println!("imported profile: {}", profile_name),
                        Err(e) => eprintln!("failed to save: {}", e),
                    },
                    Err(e) => {
                        eprintln!("import failed: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!(
                    "specify --all to import all profiles, or --as <name> for a single profile"
                );
                std::process::exit(1);
            }
        }
        Commands::Export { profile, format } => match giantd::config::load_profile(&profile) {
            Ok(loaded) => {
                let raw = giantd::config::ProfileRaw {
                    meta: loaded.meta,
                    rules: loaded.rules.iter().map(|r| r.to_raw()).collect(),
                };
                match format.as_str() {
                    "toml" => match giantd::convert::export_toml(&raw) {
                        Ok(content) => print!("{}", content),
                        Err(e) => eprintln!("export failed: {}", e),
                    },
                    "mitmproxy" => print!("{}", giantd::convert::export_mitmproxy_addon(&raw)),
                    _ => eprintln!("unknown format: {}. supported: toml, mitmproxy", format),
                }
            }
            Err(e) => {
                eprintln!("failed to load profile '{}': {}", profile, e);
                std::process::exit(1);
            }
        },
        Commands::Daemon { action } => match action {
            DaemonAction::Start => {
                if client.is_daemon_running() {
                    println!("daemon already running");
                    return;
                }
                let giantd = which_giantd();
                match std::process::Command::new(&giantd)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    Ok(child) => println!("daemon started (pid {})", child.id()),
                    Err(e) => eprintln!("failed to start daemon: {}", e),
                }
            }
            DaemonAction::Stop => {
                if !client.is_daemon_running() {
                    println!("daemon not running");
                    return;
                }
                let _ = client.post("/stop", None).await;
                let config_dir = dirs::home_dir().unwrap().join(".giant-proxy");
                if let Ok(Some(pid)) = giantd::pid::read_pid(&config_dir) {
                    let _ = std::process::Command::new("kill")
                        .arg(pid.to_string())
                        .status();
                }
                println!("daemon stopped");
            }
            DaemonAction::Install => {
                let os = std::env::consts::OS;
                match os {
                    "macos" => {
                        let dest = dirs::home_dir()
                            .unwrap()
                            .join("Library/LaunchAgents/com.giantproxy.daemon.plist");
                        std::fs::write(&dest, LAUNCHD_PLIST).expect("failed to write plist");
                        let status = std::process::Command::new("launchctl")
                            .args(["load", &dest.to_string_lossy()])
                            .status();
                        match status {
                            Ok(s) if s.success() => println!("daemon service installed and loaded"),
                            _ => eprintln!("launchctl load failed"),
                        }
                    }
                    "linux" => {
                        let dest = dirs::home_dir()
                            .unwrap()
                            .join(".config/systemd/user/giantd.service");
                        std::fs::create_dir_all(dest.parent().unwrap())
                            .expect("failed to create systemd dir");
                        std::fs::write(&dest, SYSTEMD_UNIT).expect("failed to write service");
                        let _ = std::process::Command::new("systemctl")
                            .args(["--user", "daemon-reload"])
                            .status();
                        let _ = std::process::Command::new("systemctl")
                            .args(["--user", "enable", "giantd"])
                            .status();
                        let _ = std::process::Command::new("systemctl")
                            .args(["--user", "start", "giantd"])
                            .status();
                        println!("daemon service installed and started");
                    }
                    _ => eprintln!("unsupported OS: {}", os),
                }
            }
            DaemonAction::Uninstall => {
                let os = std::env::consts::OS;
                match os {
                    "macos" => {
                        let plist = dirs::home_dir()
                            .unwrap()
                            .join("Library/LaunchAgents/com.giantproxy.daemon.plist");
                        if plist.exists() {
                            let _ = std::process::Command::new("launchctl")
                                .args(["unload", &plist.to_string_lossy()])
                                .status();
                            let _ = std::fs::remove_file(&plist);
                        }
                        println!("daemon service uninstalled");
                    }
                    "linux" => {
                        let _ = std::process::Command::new("systemctl")
                            .args(["--user", "stop", "giantd"])
                            .status();
                        let _ = std::process::Command::new("systemctl")
                            .args(["--user", "disable", "giantd"])
                            .status();
                        let unit = dirs::home_dir()
                            .unwrap()
                            .join(".config/systemd/user/giantd.service");
                        if unit.exists() {
                            let _ = std::fs::remove_file(&unit);
                        }
                        let _ = std::process::Command::new("systemctl")
                            .args(["--user", "daemon-reload"])
                            .status();
                        println!("daemon service uninstalled");
                    }
                    _ => eprintln!("unsupported OS: {}", os),
                }
            }
        },
        Commands::Uninstall => {
            println!("this will remove:");
            println!("  - daemon service (launchd/systemd)");
            println!("  - CA certificate from trust store");
            println!("  - ~/.giant-proxy/ directory");
            println!();
            print!("continue? [y/N] ");
            use std::io::Write;
            std::io::stdout().flush().unwrap();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("cancelled");
                return;
            }

            if client.is_daemon_running() {
                let _ = client.post("/stop", None).await;
                let config_dir = dirs::home_dir().unwrap().join(".giant-proxy");
                if let Ok(Some(pid)) = giantd::pid::read_pid(&config_dir) {
                    let _ = std::process::Command::new("kill")
                        .arg(pid.to_string())
                        .status();
                }
            }

            let os = std::env::consts::OS;
            match os {
                "macos" => {
                    let plist = dirs::home_dir()
                        .unwrap()
                        .join("Library/LaunchAgents/com.giantproxy.daemon.plist");
                    if plist.exists() {
                        let _ = std::process::Command::new("launchctl")
                            .args(["unload", &plist.to_string_lossy()])
                            .status();
                        let _ = std::fs::remove_file(&plist);
                    }
                    let ca_path = dirs::home_dir()
                        .unwrap()
                        .join(".giant-proxy/ca/giant-proxy-ca.pem");
                    if ca_path.exists() {
                        let _ = std::process::Command::new("sudo")
                            .args([
                                "security",
                                "remove-trusted-cert",
                                "-d",
                                &ca_path.to_string_lossy(),
                            ])
                            .status();
                    }
                }
                "linux" => {
                    let _ = std::process::Command::new("systemctl")
                        .args(["--user", "stop", "giantd"])
                        .status();
                    let _ = std::process::Command::new("systemctl")
                        .args(["--user", "disable", "giantd"])
                        .status();
                    let unit = dirs::home_dir()
                        .unwrap()
                        .join(".config/systemd/user/giantd.service");
                    if unit.exists() {
                        let _ = std::fs::remove_file(&unit);
                    }
                    let _ = std::process::Command::new("sudo")
                        .args([
                            "rm",
                            "-f",
                            "/usr/local/share/ca-certificates/giant-proxy-ca.crt",
                        ])
                        .status();
                    let _ = std::process::Command::new("sudo")
                        .args(["update-ca-certificates"])
                        .status();
                }
                _ => {}
            }

            let config_dir = dirs::home_dir().unwrap().join(".giant-proxy");
            if config_dir.exists() {
                std::fs::remove_dir_all(&config_dir).expect("failed to remove ~/.giant-proxy");
            }

            println!("giant-proxy uninstalled");
        }
        Commands::Version => {
            println!("giant-proxy {}", env!("CARGO_PKG_VERSION"));
        }
    }
}

async fn ensure_daemon(client: &DaemonClient) {
    if client.is_daemon_running() && client.get("/health").await.is_ok() {
        return;
    }

    eprintln!("daemon not running. start it with: giantd --foreground");
    std::process::exit(1);
}

fn which_giantd() -> String {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("giantd")))
        .filter(|p| p.exists());
    match exe_dir {
        Some(p) => p.to_string_lossy().to_string(),
        None => "giantd".to_string(),
    }
}
