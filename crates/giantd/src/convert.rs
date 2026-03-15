use crate::config::{self, ProfileMeta, ProfileRaw};
use crate::errors::{GiantError, Result};
use crate::rules::{MatchRule, RuleRaw, Target};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct LegacyRule {
    pub id: String,
    pub enabled: bool,
    pub regex: String,
    pub host: String,
    pub port: u16,
    #[serde(default = "default_http")]
    pub scheme: String,
}

fn default_http() -> String {
    "http".to_string()
}

pub fn import_legacy(path: &Path) -> Result<HashMap<String, ProfileRaw>> {
    let content = std::fs::read_to_string(path)?;
    let legacy: HashMap<String, Vec<LegacyRule>> = serde_json::from_str(&content)
        .map_err(|e| GiantError::ConfigError(format!("invalid legacy JSON: {}", e)))?;

    let mut profiles = HashMap::new();
    for (name, rules) in legacy {
        let rule_raws: Vec<RuleRaw> = rules
            .into_iter()
            .map(|lr| RuleRaw {
                id: lr.id,
                enabled: lr.enabled,
                match_rule: MatchRule {
                    host: None,
                    path: None,
                    not_path: None,
                    method: None,
                    regex: Some(lr.regex),
                },
                target: Target {
                    host: lr.host,
                    port: lr.port,
                    scheme: lr.scheme,
                    path: None,
                },
                preserve_host: true,
                priority: 0,
            })
            .collect();

        profiles.insert(
            name.clone(),
            ProfileRaw {
                meta: ProfileMeta {
                    name: name.clone(),
                    description: Some("imported from legacy rules.json".to_string()),
                    format_version: 1,
                },
                rules: rule_raws,
            },
        );
    }
    Ok(profiles)
}

pub fn import_legacy_profile(path: &Path, profile_name: &str) -> Result<ProfileRaw> {
    let profiles = import_legacy(path)?;
    profiles
        .into_iter()
        .find(|(name, _)| name == profile_name)
        .map(|(_, profile)| profile)
        .ok_or_else(|| {
            GiantError::ConfigError(format!(
                "profile '{}' not found in legacy file",
                profile_name
            ))
        })
}

pub fn export_toml(profile: &ProfileRaw) -> Result<String> {
    toml::to_string_pretty(profile).map_err(|e| GiantError::ConfigError(e.to_string()))
}

pub fn save_profile(profile: &ProfileRaw) -> Result<()> {
    let dir = config::config_dir().join("profiles");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.toml", profile.meta.name));
    let content = export_toml(profile)?;
    std::fs::write(&path, content)?;
    Ok(())
}

pub fn export_mitmproxy_addon(profile: &ProfileRaw) -> String {
    let mut rules_code = String::new();
    for rule in &profile.rules {
        if !rule.enabled {
            continue;
        }
        if let Some(ref regex) = rule.match_rule.regex {
            rules_code.push_str(&format!(
                "    ({:?}, \"{}\", {}, \"{}\"),\n",
                regex, rule.target.host, rule.target.port, rule.target.scheme
            ));
        }
    }

    format!(
        r#"import re
from mitmproxy import http

RULES = [
{}]

def request(flow: http.HTTPFlow) -> None:
    url = flow.request.pretty_url
    for pattern, host, port, scheme in RULES:
        if re.search(pattern, url):
            flow.request.host = host
            flow.request.port = port
            flow.request.scheme = scheme
            break
"#,
        rules_code
    )
}
