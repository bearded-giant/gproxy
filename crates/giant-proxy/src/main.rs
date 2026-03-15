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
enum Commands {
    /// show proxy and daemon status
    Status,
    /// start proxy with a profile
    On {
        #[arg(long)]
        profile: Option<String>,
    },
    /// stop proxy
    Off,
    /// print shell env vars for proxy
    Env,
    /// manage profiles
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// manage rules within a profile
    Rule {
        #[command(subcommand)]
        action: RuleAction,
    },
    /// manage the daemon process
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// run diagnostics
    Doctor {
        #[arg(long)]
        fix: bool,
    },
    /// initialize config directory
    Init,
    /// remove giant-proxy completely
    Uninstall,
    /// print version
    Version,
}

#[derive(Subcommand)]
enum ProfileAction {
    /// list all profiles
    #[command(alias = "ls")]
    List,
    /// show profile details and rules
    Show { name: String },
    /// create an empty profile
    Create { name: String },
    /// delete a profile
    Delete { name: String },
    /// rename a profile
    Rename {
        old_name: String,
        new_name: String,
    },
    /// set profile display order
    Reorder {
        /// profile names in desired order
        names: Vec<String>,
    },
    /// import from file (auto-detects proxyman or legacy format)
    Import {
        file: String,
        #[arg(long)]
        all: bool,
    },
    /// import all rules directly from local Proxyman install
    ImportProxyman,
    /// export a profile
    Export {
        name: String,
        #[arg(long, default_value = "toml")]
        format: String,
    },
}

#[derive(Subcommand)]
enum RuleAction {
    /// list rules in a profile
    #[command(alias = "ls")]
    List { profile: String },
    /// show rule details
    Show { profile: String, rule_id: String },
    /// add a rule to a profile
    Add {
        profile: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        regex: Option<String>,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        path: Option<String>,
        #[arg(long, default_value = "localhost")]
        target_host: String,
        #[arg(long, default_value = "3000")]
        target_port: u16,
        #[arg(long, default_value = "http")]
        target_scheme: String,
        #[arg(long)]
        disabled: bool,
    },
    /// delete a rule from a profile
    Delete { profile: String, rule_id: String },
    /// toggle a rule enabled/disabled
    Toggle { profile: String, rule_id: String },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// start the daemon
    Start,
    /// stop the daemon
    Stop,
    /// install as system service
    Install,
    /// remove system service
    Uninstall,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let client = DaemonClient::new();

    match cli.command {
        Commands::Status => cmd_status(&client).await,
        Commands::On { profile } => {
            ensure_daemon(&client).await;
            let name = profile.unwrap_or_else(|| "preprod".to_string());
            match client.post(&format!("/use/{}", name), None).await {
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
        Commands::Env => cmd_env(&client).await,
        Commands::Profile { action } => cmd_profile(action),
        Commands::Rule { action } => cmd_rule(action),
        Commands::Daemon { action } => cmd_daemon(action, &client).await,
        Commands::Doctor { .. } => cmd_doctor(&client),
        Commands::Init => cmd_init(),
        Commands::Uninstall => cmd_uninstall(&client).await,
        Commands::Version => println!("giant-proxy {}", env!("CARGO_PKG_VERSION")),
    }
}

// -- status --

async fn cmd_status(client: &DaemonClient) {
    if !client.is_daemon_running() {
        println!("  daemon:   stopped");
        println!("  proxy:    off");
        println!("  profile:  -");
        let profiles = giantd::config::list_profiles().unwrap_or_default();
        println!("  profiles: {}", if profiles.is_empty() { "(none)".to_string() } else { profiles.join(", ") });
        return;
    }
    match client.get("/status").await {
        Ok(resp) => {
            let running = resp.get("running").and_then(|v| v.as_bool()).unwrap_or(false);
            let profile = resp.get("profile").and_then(|v| v.as_str()).unwrap_or("-");
            let addr = resp.get("listen_addr").and_then(|v| v.as_str()).unwrap_or("-");
            let mode = resp.get("routing_mode").and_then(|v| v.as_str()).unwrap_or("-");
            let rules = resp.get("rules").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
            let profiles = giantd::config::list_profiles().unwrap_or_default();

            println!("  daemon:   running");
            println!("  proxy:    {}", if running { "on" } else { "off" });
            println!("  profile:  {}", profile);
            println!("  rules:    {}", rules);
            println!("  listen:   {}", addr);
            println!("  routing:  {}", mode);
            println!("  profiles: {}", if profiles.is_empty() { "(none)".to_string() } else { profiles.join(", ") });
        }
        Err(e) => eprintln!("error: {}", e),
    }
}

// -- profile --

fn cmd_profile(action: ProfileAction) {
    match action {
        ProfileAction::List => {
            let names = giantd::config::list_profiles().unwrap_or_default();
            if names.is_empty() {
                println!("no profiles found");
                return;
            }
            for name in &names {
                match giantd::config::load_profile(name) {
                    Ok(p) => println!("  {} ({} rules)", name, p.rules.len()),
                    Err(_) => println!("  {} (error loading)", name),
                }
            }
        }
        ProfileAction::Show { name } => {
            match giantd::config::load_profile(&name) {
                Ok(p) => {
                    println!("profile: {}", name);
                    if let Some(desc) = &p.meta.description {
                        println!("  desc: {}", desc);
                    }
                    println!("  rules: {}", p.rules.len());
                    println!();
                    for r in &p.rules {
                        let status = if r.enabled { "on " } else { "off" };
                        let match_str = r.match_rule.regex.as_deref()
                            .or(r.match_rule.host.as_deref())
                            .unwrap_or("-");
                        println!("  [{}] {}", status, r.id);
                        println!("        match:  {}", match_str);
                        println!("        target: {}://{}:{}", r.target.scheme, r.target.host, r.target.port);
                    }
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        ProfileAction::Create { name } => {
            let profile = giantd::config::ProfileRaw {
                meta: giantd::config::ProfileMeta {
                    name: name.clone(),
                    description: None,
                    format_version: 1,
                },
                rules: vec![],
            };
            match giantd::config::write_profile(&profile) {
                Ok(()) => println!("created profile: {}", name),
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        ProfileAction::Delete { name } => {
            let path = giantd::config::config_dir()
                .join("profiles")
                .join(format!("{}.toml", name));
            if !path.exists() {
                eprintln!("profile '{}' not found", name);
                std::process::exit(1);
            }
            std::fs::remove_file(&path).expect("failed to delete profile");
            println!("deleted profile: {}", name);
        }
        ProfileAction::Rename { old_name, new_name } => {
            match giantd::config::rename_profile(&old_name, &new_name) {
                Ok(()) => println!("renamed '{}' -> '{}'", old_name, new_name),
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            }
        }
        ProfileAction::Reorder { names } => {
            match giantd::config::save_profile_order(&names) {
                Ok(()) => println!("profile order saved"),
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            }
        }
        ProfileAction::Import { file, all } => {
            let path = std::path::Path::new(&file);
            if !path.exists() {
                eprintln!("file not found: {}", file);
                std::process::exit(1);
            }
            if all {
                match giantd::convert::import_auto(path) {
                    Ok(profiles) => {
                        for (pname, profile) in &profiles {
                            match giantd::convert::save_profile(profile) {
                                Ok(()) => println!("  imported: {}", pname),
                                Err(e) => eprintln!("  failed {}: {}", pname, e),
                            }
                        }
                        println!("{} profiles imported", profiles.len());
                    }
                    Err(e) => {
                        eprintln!("import failed: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("specify --all to import all profiles from file");
                std::process::exit(1);
            }
        }
        ProfileAction::ImportProxyman => {
            let path = dirs::home_dir()
                .expect("home dir")
                .join("Library/Application Support/com.proxyman.NSProxy/user-data/MapRemoteService");
            if !path.exists() {
                eprintln!("proxyman config not found at {}", path.display());
                std::process::exit(1);
            }
            match giantd::convert::import_proxyman(&path) {
                Ok(profiles) => {
                    for (pname, profile) in &profiles {
                        match giantd::convert::save_profile(profile) {
                            Ok(()) => println!("  imported: {} ({} rules)", pname, profile.rules.len()),
                            Err(e) => eprintln!("  failed {}: {}", pname, e),
                        }
                    }
                    println!("{} profiles imported", profiles.len());
                }
                Err(e) => {
                    eprintln!("import failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        ProfileAction::Export { name, format } => {
            match giantd::config::load_profile(&name) {
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
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

// -- rule --

fn cmd_rule(action: RuleAction) {
    match action {
        RuleAction::List { profile } => {
            match giantd::config::load_profile(&profile) {
                Ok(p) => {
                    if p.rules.is_empty() {
                        println!("no rules in profile '{}'", profile);
                        return;
                    }
                    for r in &p.rules {
                        let status = if r.enabled { "on " } else { "off" };
                        let match_str = r.match_rule.regex.as_deref()
                            .or(r.match_rule.host.as_deref())
                            .unwrap_or("-");
                        let target = format!("{}://{}:{}", r.target.scheme, r.target.host, r.target.port);
                        println!("  [{}] {:30} {} -> {}", status, r.id, match_str, target);
                    }
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        RuleAction::Show { profile, rule_id } => {
            match giantd::config::load_profile(&profile) {
                Ok(p) => {
                    match p.rules.iter().find(|r| r.id == rule_id) {
                        Some(r) => {
                            println!("rule: {}", r.id);
                            println!("  enabled: {}", r.enabled);
                            println!("  preserve_host: {}", r.preserve_host);
                            if let Some(ref re) = r.match_rule.regex {
                                println!("  match.regex: {}", re);
                            }
                            if let Some(ref h) = r.match_rule.host {
                                println!("  match.host: {}", h);
                            }
                            if let Some(ref p) = r.match_rule.path {
                                println!("  match.path: {}", p);
                            }
                            if let Some(ref m) = r.match_rule.method {
                                println!("  match.method: {}", m);
                            }
                            println!("  target: {}://{}:{}", r.target.scheme, r.target.host, r.target.port);
                        }
                        None => {
                            eprintln!("rule '{}' not found in profile '{}'", rule_id, profile);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        RuleAction::Add {
            profile, id, regex, host, path,
            target_host, target_port, target_scheme, disabled,
        } => {
            let mut profile_raw = match load_profile_raw(&profile) {
                Ok(p) => p,
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            };

            if profile_raw.rules.iter().any(|r| r.id == id) {
                eprintln!("rule '{}' already exists in profile '{}'", id, profile);
                std::process::exit(1);
            }

            let rule = giantd::rules::RuleRaw {
                id: id.clone(),
                enabled: !disabled,
                match_rule: giantd::rules::MatchRule {
                    host,
                    path,
                    not_path: None,
                    method: None,
                    regex,
                },
                target: giantd::rules::Target {
                    host: target_host,
                    port: target_port,
                    scheme: target_scheme,
                    path: None,
                },
                preserve_host: true,
                priority: 0,
            };

            profile_raw.rules.push(rule);
            match giantd::config::write_profile(&profile_raw) {
                Ok(()) => println!("added rule '{}' to profile '{}'", id, profile),
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            }
        }
        RuleAction::Delete { profile, rule_id } => {
            let mut profile_raw = match load_profile_raw(&profile) {
                Ok(p) => p,
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            };

            let before = profile_raw.rules.len();
            profile_raw.rules.retain(|r| r.id != rule_id);
            if profile_raw.rules.len() == before {
                eprintln!("rule '{}' not found in profile '{}'", rule_id, profile);
                std::process::exit(1);
            }

            match giantd::config::write_profile(&profile_raw) {
                Ok(()) => println!("deleted rule '{}' from profile '{}'", rule_id, profile),
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            }
        }
        RuleAction::Toggle { profile, rule_id } => {
            let mut profile_raw = match load_profile_raw(&profile) {
                Ok(p) => p,
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            };

            match profile_raw.rules.iter_mut().find(|r| r.id == rule_id) {
                Some(r) => {
                    r.enabled = !r.enabled;
                    let state = if r.enabled { "enabled" } else { "disabled" };
                    match giantd::config::write_profile(&profile_raw) {
                        Ok(()) => println!("{} rule '{}' in profile '{}'", state, rule_id, profile),
                        Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
                    }
                }
                None => {
                    eprintln!("rule '{}' not found in profile '{}'", rule_id, profile);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn load_profile_raw(name: &str) -> Result<giantd::config::ProfileRaw, String> {
    let p = giantd::config::load_profile(name).map_err(|e| e.to_string())?;
    Ok(giantd::config::ProfileRaw {
        meta: p.meta,
        rules: p.rules.iter().map(|r| r.to_raw()).collect(),
    })
}

// -- daemon --

async fn cmd_daemon(action: DaemonAction, client: &DaemonClient) {
    match action {
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
    }
}

// -- other commands --

async fn cmd_env(client: &DaemonClient) {
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

fn cmd_doctor(client: &DaemonClient) {
    println!("running diagnostics...");
    let config_dir = dirs::home_dir()
        .expect("home directory must exist")
        .join(".giant-proxy");
    let ca_cert = config_dir.join("ca").join("giant-proxy-ca.pem");
    println!("  CA cert: {}", if ca_cert.exists() { "found" } else { "MISSING" });
    let ca_key = config_dir.join("ca").join("giant-proxy-ca-key.pem");
    println!("  CA key:  {}", if ca_key.exists() { "found" } else { "MISSING" });
    println!("  daemon:  {}", if client.is_daemon_running() { "running" } else { "stopped" });
}

fn cmd_init() {
    println!("initializing giant-proxy...");
    let config_dir = dirs::home_dir()
        .expect("home directory must exist")
        .join(".giant-proxy");
    std::fs::create_dir_all(&config_dir).expect("failed to create config dir");
    std::fs::create_dir_all(config_dir.join("profiles")).expect("failed to create profiles dir");
    std::fs::create_dir_all(config_dir.join("logs")).expect("failed to create logs dir");
    println!("config directory: {}", config_dir.display());
    println!("run `giant-proxy on` to start the proxy");
}

async fn cmd_uninstall(client: &DaemonClient) {
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
                    .args(["security", "remove-trusted-cert", "-d", &ca_path.to_string_lossy()])
                    .status();
            }
        }
        "linux" => {
            let _ = std::process::Command::new("systemctl").args(["--user", "stop", "giantd"]).status();
            let _ = std::process::Command::new("systemctl").args(["--user", "disable", "giantd"]).status();
            let unit = dirs::home_dir().unwrap().join(".config/systemd/user/giantd.service");
            if unit.exists() { let _ = std::fs::remove_file(&unit); }
            let _ = std::process::Command::new("sudo").args(["rm", "-f", "/usr/local/share/ca-certificates/giant-proxy-ca.crt"]).status();
            let _ = std::process::Command::new("sudo").args(["update-ca-certificates"]).status();
        }
        _ => {}
    }

    let config_dir = dirs::home_dir().unwrap().join(".giant-proxy");
    if config_dir.exists() {
        std::fs::remove_dir_all(&config_dir).expect("failed to remove ~/.giant-proxy");
    }
    println!("giant-proxy uninstalled");
}

async fn ensure_daemon(client: &DaemonClient) {
    if client.is_daemon_running() && client.get("/health").await.is_ok() {
        return;
    }
    eprintln!("daemon not running. start it with: giant-proxy daemon start");
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
