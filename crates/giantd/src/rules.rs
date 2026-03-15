use glob_match::glob_match;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuleRaw {
    pub id: String,
    pub enabled: bool,
    #[serde(rename = "match")]
    pub match_rule: MatchRule,
    pub target: Target,
    #[serde(default = "default_true")]
    pub preserve_host: bool,
    #[serde(default)]
    pub priority: u32,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub id: String,
    pub enabled: bool,
    pub match_rule: MatchRule,
    pub target: Target,
    pub preserve_host: bool,
    pub priority: u32,
    pub compiled_regex: Option<Regex>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MatchRule {
    pub host: Option<String>,
    pub path: Option<String>,
    pub not_path: Option<String>,
    pub method: Option<String>,
    pub regex: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Target {
    pub host: String,
    pub port: u16,
    #[serde(default = "default_http")]
    pub scheme: String,
    pub path: Option<String>,
}

fn default_true() -> bool {
    true
}
fn default_http() -> String {
    "http".to_string()
}

impl Rule {
    pub fn from_raw(raw: RuleRaw) -> std::result::Result<Self, regex::Error> {
        let compiled_regex = match &raw.match_rule.regex {
            Some(re_str) => Some(Regex::new(re_str)?),
            None => None,
        };
        Ok(Self {
            id: raw.id,
            enabled: raw.enabled,
            match_rule: raw.match_rule,
            target: raw.target,
            preserve_host: raw.preserve_host,
            priority: raw.priority,
            compiled_regex,
        })
    }

    pub fn matches(
        &self,
        uri: &http::Uri,
        headers: &http::HeaderMap,
        method: &http::Method,
    ) -> bool {
        if let Some(ref m) = self.match_rule.method {
            if m != "ANY" && m != method.as_str() {
                return false;
            }
        }

        if let Some(ref re) = self.compiled_regex {
            let full_url = uri.to_string();
            return re.is_match(&full_url);
        }

        // check both Host header (HTTP/1.1) and URI authority (HTTP/2 :authority)
        let host = headers
            .get("host")
            .and_then(|h| h.to_str().ok())
            .map(|h| h.split(':').next().unwrap_or(h))
            .or_else(|| uri.authority().map(|a| a.host()))
            .unwrap_or("");

        let host_pattern = self.match_rule.host.as_deref().unwrap_or("*");
        if !glob_match(host_pattern, host) {
            return false;
        }

        let path = uri.path();
        if let Some(ref p) = self.match_rule.path {
            if !glob_match(p, path) {
                return false;
            }
        }
        if let Some(ref np) = self.match_rule.not_path {
            if glob_match(np, path) {
                return false;
            }
        }

        true
    }

    pub fn rewrite_uri(&self, original: &http::Uri) -> http::Uri {
        let path_and_query = if let Some(ref target_path) = self.target.path {
            target_path.clone()
        } else {
            original
                .path_and_query()
                .map(|pq| pq.to_string())
                .unwrap_or_else(|| "/".to_string())
        };

        http::Uri::builder()
            .scheme(self.target.scheme.as_str())
            .authority(format!("{}:{}", self.target.host, self.target.port))
            .path_and_query(path_and_query)
            .build()
            .expect("valid rewritten URI")
    }

    pub fn to_raw(&self) -> RuleRaw {
        RuleRaw {
            id: self.id.clone(),
            enabled: self.enabled,
            match_rule: self.match_rule.clone(),
            target: self.target.clone(),
            preserve_host: self.preserve_host,
            priority: self.priority,
        }
    }
}
