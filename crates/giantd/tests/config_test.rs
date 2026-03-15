use giantd::config::{AppConfig, Profile, ProfileRaw};
use giantd::rules::{MatchRule, RuleRaw, Target};

#[test]
fn parse_full_config() {
    let toml_str = r#"
version = 1
listen_port = 9090
pac_port = 9999
log_level = "debug"
log_max_size_mb = 20
log_max_files = 10
auto_start = true
default_profile = "preprod"
routing_mode = "system"
browser = "chrome"
bypass_hosts = ["localhost", "127.0.0.1"]
"#;
    let config: AppConfig = toml::from_str(toml_str).expect("should parse");
    assert_eq!(config.listen_port, 9090);
    assert_eq!(config.pac_port, 9999);
    assert_eq!(config.log_level, "debug");
    assert_eq!(config.log_max_size_mb, 20);
    assert_eq!(config.log_max_files, 10);
    assert!(config.auto_start);
    assert_eq!(config.default_profile, Some("preprod".to_string()));
    assert_eq!(config.routing_mode, "system");
    assert_eq!(config.browser, Some("chrome".to_string()));
    assert_eq!(config.bypass_hosts, vec!["localhost", "127.0.0.1"]);
}

#[test]
fn parse_minimal_config_uses_defaults() {
    let toml_str = "";
    let config: AppConfig = toml::from_str(toml_str).expect("should parse empty config");
    assert_eq!(config.listen_port, 8080);
    assert_eq!(config.pac_port, 9876);
    assert_eq!(config.log_level, "info");
    assert_eq!(config.log_max_size_mb, 10);
    assert_eq!(config.log_max_files, 5);
    assert!(!config.auto_start);
    assert_eq!(config.default_profile, None);
    assert_eq!(config.routing_mode, "manual");
    assert_eq!(config.browser, None);
    assert!(config.bypass_hosts.is_empty());
}

#[test]
fn parse_profile_toml() {
    let toml_str = r#"
[meta]
name = "test"
description = "Test profile"
format_version = 1

[[rules]]
id = "rule1"
enabled = true
preserve_host = true

[rules.match]
host = "*.example.com"
path = "/api/*"

[rules.target]
host = "localhost"
port = 3000
scheme = "http"
"#;
    let raw: ProfileRaw = toml::from_str(toml_str).expect("should parse profile");
    assert_eq!(raw.meta.name, "test");
    assert_eq!(raw.rules.len(), 1);
    assert_eq!(raw.rules[0].id, "rule1");
    assert!(raw.rules[0].enabled);

    let profile = Profile::from_raw(raw).expect("should compile profile");
    assert_eq!(profile.rules.len(), 1);
    assert_eq!(profile.rules[0].id, "rule1");
}

#[test]
fn parse_profile_with_regex_rule() {
    let toml_str = r#"
[meta]
name = "regex_test"
format_version = 1

[[rules]]
id = "regex_rule"
enabled = true

[rules.match]
regex = "^https://api-v[0-9]+\\.example\\.com/.*"

[rules.target]
host = "localhost"
port = 8000
"#;
    let raw: ProfileRaw = toml::from_str(toml_str).expect("should parse profile with regex");
    let profile = Profile::from_raw(raw).expect("should compile regex rule");
    assert!(profile.rules[0].compiled_regex.is_some());
}

#[test]
fn reject_duplicate_rule_ids() {
    let rules = vec![
        RuleRaw {
            id: "same_id".to_string(),
            enabled: true,
            match_rule: MatchRule {
                host: Some("*.example.com".to_string()),
                path: None,
                not_path: None,
                method: None,
                regex: None,
            },
            target: Target {
                host: "localhost".to_string(),
                port: 3000,
                scheme: "http".to_string(),
                path: None,
            },
            preserve_host: true,
            priority: 0,
        },
        RuleRaw {
            id: "same_id".to_string(),
            enabled: true,
            match_rule: MatchRule {
                host: Some("*.other.com".to_string()),
                path: None,
                not_path: None,
                method: None,
                regex: None,
            },
            target: Target {
                host: "localhost".to_string(),
                port: 4000,
                scheme: "http".to_string(),
                path: None,
            },
            preserve_host: true,
            priority: 0,
        },
    ];

    let raw = ProfileRaw {
        meta: giantd::config::ProfileMeta {
            name: "test".to_string(),
            description: None,
            format_version: 1,
        },
        rules,
    };

    assert!(Profile::from_raw(raw).is_err());
}

#[test]
fn config_defaults() {
    let config = AppConfig {
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
    };
    assert_eq!(config.listen_port, 8080);
    assert_eq!(config.pac_port, 9876);
    assert_eq!(config.routing_mode, "manual");
}
