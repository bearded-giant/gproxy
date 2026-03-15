use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::errors::{GiantError, Result};
use crate::rules::{Rule, RuleRaw};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default = "default_listen_port")]
    pub listen_port: u16,
    #[serde(default = "default_pac_port")]
    pub pac_port: u16,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_log_max_size_mb")]
    pub log_max_size_mb: u32,
    #[serde(default = "default_log_max_files")]
    pub log_max_files: u32,
    #[serde(default)]
    pub auto_start: bool,
    pub default_profile: Option<String>,
    #[serde(default = "default_routing_mode")]
    pub routing_mode: String,
    pub browser: Option<String>,
    #[serde(default)]
    pub bypass_hosts: Vec<String>,
}

fn default_version() -> u32 {
    1
}
fn default_listen_port() -> u16 {
    8080
}
fn default_pac_port() -> u16 {
    9876
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_log_max_size_mb() -> u32 {
    10
}
fn default_log_max_files() -> u32 {
    5
}
fn default_routing_mode() -> String {
    "manual".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProfileMeta {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_version")]
    pub format_version: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProfileRaw {
    pub meta: ProfileMeta,
    pub rules: Vec<RuleRaw>,
}

#[derive(Debug, Clone)]
pub struct Profile {
    pub meta: ProfileMeta,
    pub rules: Vec<Rule>,
}

impl Profile {
    pub fn from_raw(raw: ProfileRaw) -> Result<Self> {
        let mut ids = std::collections::HashSet::new();
        let mut rules = Vec::new();
        for rule_raw in raw.rules {
            if !ids.insert(rule_raw.id.clone()) {
                return Err(GiantError::RuleError(format!(
                    "duplicate rule id: {}",
                    rule_raw.id
                )));
            }
            rules.push(Rule::from_raw(rule_raw).map_err(|e| GiantError::RuleError(e.to_string()))?);
        }
        Ok(Self {
            meta: raw.meta,
            rules,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonState {
    pub running: bool,
    pub active_profile: Option<String>,
    #[serde(default)]
    pub also_profiles: Vec<String>,
    pub listen_addr: String,
    pub routing_mode: String,
    pub pid: u32,
    pub started_at: String,
    pub version: String,
    pub rules: Vec<RuleState>,
    #[serde(default)]
    pub proxy_services: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleState {
    pub id: String,
    pub enabled: bool,
    pub matched_count: u64,
}

pub fn config_dir() -> PathBuf {
    dirs::home_dir()
        .expect("home directory must exist")
        .join(".giant-proxy")
}

pub fn load_config() -> Result<AppConfig> {
    let path = config_dir().join("config.toml");
    if !path.exists() {
        return Ok(AppConfig {
            version: 1,
            listen_port: 8080,
            pac_port: 9876,
            log_level: "info".to_string(),
            log_max_size_mb: 10,
            log_max_files: 5,
            auto_start: false,
            default_profile: None,
            routing_mode: "manual".to_string(),
            browser: None,
            bypass_hosts: vec![],
        });
    }
    let content = std::fs::read_to_string(&path)?;
    let config: AppConfig = toml::from_str(&content)?;
    Ok(config)
}

pub fn load_profile(name: &str) -> Result<Profile> {
    let path = config_dir().join("profiles").join(format!("{}.toml", name));
    let content = std::fs::read_to_string(&path)?;
    let raw: ProfileRaw = toml::from_str(&content)
        .map_err(|e| GiantError::ConfigError(format!("failed to parse profile {}: {}", name, e)))?;
    Profile::from_raw(raw)
}

pub fn list_profiles() -> Result<Vec<String>> {
    let dir = config_dir().join("profiles");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut profiles = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                profiles.push(stem.to_string());
            }
        }
    }
    profiles.sort();
    Ok(profiles)
}

pub fn write_profile(profile: &ProfileRaw) -> Result<()> {
    let dir = config_dir().join("profiles");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.toml", profile.meta.name));
    let content =
        toml::to_string_pretty(profile).map_err(|e| GiantError::ConfigError(e.to_string()))?;
    std::fs::write(&path, content)?;
    Ok(())
}

pub fn write_state(state: &DaemonState) -> Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let tmp_path = dir.join(".state.json.tmp");
    let final_path = dir.join("state.json");
    let content =
        serde_json::to_string_pretty(state).map_err(|e| GiantError::ConfigError(e.to_string()))?;
    std::fs::write(&tmp_path, &content)?;
    std::fs::rename(&tmp_path, &final_path)?;
    Ok(())
}

pub fn read_state() -> Result<Option<DaemonState>> {
    let path = config_dir().join("state.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let state: DaemonState =
        serde_json::from_str(&content).map_err(|e| GiantError::ConfigError(e.to_string()))?;
    Ok(Some(state))
}

pub fn migrate_config() -> Result<()> {
    let path = config_dir().join("config.toml");
    if !path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(&path)?;
    let config: AppConfig = toml::from_str(&content)?;

    if config.version < 1 {
        let backup = config_dir().join("config.toml.bak");
        std::fs::copy(&path, &backup)?;
        let migrated = AppConfig {
            version: 1,
            ..config
        };
        let new_content = toml::to_string_pretty(&migrated)
            .map_err(|e| GiantError::ConfigError(e.to_string()))?;
        std::fs::write(&path, new_content)?;
    }

    Ok(())
}
