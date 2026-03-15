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

// proxyman map remote config import

#[derive(Debug, Deserialize)]
struct ProxymanMethod {
    #[serde(default)]
    any: Option<serde_json::Value>,
    #[serde(default)]
    get: Option<serde_json::Value>,
    #[serde(default)]
    post: Option<serde_json::Value>,
    #[serde(default)]
    put: Option<serde_json::Value>,
    #[serde(default)]
    delete: Option<serde_json::Value>,
    #[serde(default)]
    patch: Option<serde_json::Value>,
}

impl ProxymanMethod {
    fn to_method_string(&self) -> Option<String> {
        if self.any.is_some() {
            return None; // ANY = no filter
        }
        if self.get.is_some() { return Some("GET".to_string()); }
        if self.post.is_some() { return Some("POST".to_string()); }
        if self.put.is_some() { return Some("PUT".to_string()); }
        if self.delete.is_some() { return Some("DELETE".to_string()); }
        if self.patch.is_some() { return Some("PATCH".to_string()); }
        None
    }
}

#[derive(Debug, Deserialize)]
struct ProxymanURLComponent {
    #[serde(default)]
    scheme: String,
    #[serde(default)]
    host: String,
    #[serde(default)]
    port: String,
    #[serde(default)]
    path: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ProxymanRule {
    id: String,
    name: String,
    #[serde(default, rename = "isEnabled")]
    is_enabled: bool,
    #[serde(default, rename = "mapFromURL")]
    map_from_url: String,
    #[serde(default)]
    regex: String,
    #[serde(default)]
    method: Option<ProxymanMethod>,
    #[serde(default, rename = "preserveHostHeader")]
    preserve_host_header: bool,
    #[serde(default, rename = "preserveOriginalURL")]
    preserve_original_url: bool,
    #[serde(rename = "toURLComponent")]
    to_url_component: ProxymanURLComponent,
    #[serde(default, rename = "fromURLComponent")]
    from_url_component: Option<ProxymanURLComponent>,
}

fn slugify(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c.to_ascii_lowercase() } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn extract_profile_prefix(name: &str) -> (String, String) {
    // extract "[PREPROD] admin tools" → ("preprod", "admin_tools")
    if let Some(rest) = name.strip_prefix('[') {
        if let Some(close) = rest.find(']') {
            let prefix = rest[..close].trim().to_lowercase();
            let suffix = rest[close + 1..].trim();
            return (prefix, slugify(suffix));
        }
    }
    ("default".to_string(), slugify(name))
}

pub fn import_proxyman(path: &Path) -> Result<HashMap<String, ProfileRaw>> {
    let content = std::fs::read_to_string(path)?;
    let rules: Vec<ProxymanRule> = serde_json::from_str(&content)
        .map_err(|e| GiantError::ConfigError(format!("invalid proxyman JSON: {}", e)))?;

    let mut grouped: HashMap<String, Vec<RuleRaw>> = HashMap::new();

    for pr in rules {
        let (profile_name, rule_id) = extract_profile_prefix(&pr.name);

        let port: u16 = pr.to_url_component.port.parse().unwrap_or(80);

        let match_rule = if pr.regex == "useRegex" && !pr.map_from_url.is_empty() {
            MatchRule {
                host: None,
                path: None,
                not_path: None,
                method: pr.method.as_ref().and_then(|m| m.to_method_string()),
                // proxyman uses \/ for forward slashes (JSON legacy), strip to plain /
                regex: Some(pr.map_from_url.replace("\\/", "/")),
            }
        } else {
            // url component matching mode
            let from = pr.from_url_component.as_ref();
            let host = from
                .map(|f| &f.host)
                .filter(|h| !h.is_empty())
                .cloned();
            let path_pat = from
                .map(|f| &f.path)
                .filter(|p| !p.is_empty())
                .cloned();
            MatchRule {
                host,
                path: path_pat,
                not_path: None,
                method: pr.method.as_ref().and_then(|m| m.to_method_string()),
                regex: None,
            }
        };

        let target = Target {
            host: if pr.to_url_component.host.is_empty() {
                "localhost".to_string()
            } else {
                pr.to_url_component.host.clone()
            },
            port,
            scheme: if pr.to_url_component.scheme.is_empty() {
                "http".to_string()
            } else {
                pr.to_url_component.scheme.clone()
            },
            path: if pr.to_url_component.path.is_empty() {
                None
            } else {
                Some(pr.to_url_component.path.clone())
            },
        };

        let rule_raw = RuleRaw {
            id: rule_id,
            enabled: pr.is_enabled,
            match_rule,
            target,
            preserve_host: pr.preserve_host_header || pr.preserve_original_url,
            priority: 0,
        };

        grouped.entry(profile_name).or_default().push(rule_raw);
    }

    let mut profiles = HashMap::new();
    for (name, rules) in grouped {
        // deduplicate rule ids within a profile
        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::new();
        for mut rule in rules {
            let base = rule.id.clone();
            let mut counter = 1u32;
            while !seen.insert(rule.id.clone()) {
                counter += 1;
                rule.id = format!("{}_{}", base, counter);
            }
            deduped.push(rule);
        }

        profiles.insert(
            name.clone(),
            ProfileRaw {
                meta: ProfileMeta {
                    name: name.clone(),
                    description: Some("imported from proxyman".to_string()),
                    format_version: 1,
                },
                rules: deduped,
            },
        );
    }

    Ok(profiles)
}

pub fn import_auto(path: &Path) -> Result<HashMap<String, ProfileRaw>> {
    let content = std::fs::read_to_string(path)?;
    let val: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| GiantError::ConfigError(format!("invalid JSON: {}", e)))?;

    if val.is_array() {
        // proxyman format: array of rule objects
        return import_proxyman(path);
    }
    // legacy format: object keyed by profile name
    import_legacy(path)
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
