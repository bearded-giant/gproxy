use giantd::config::{ProfileMeta, ProfileRaw};
use giantd::rules::{MatchRule, RuleRaw, Target};

fn make_profile(rules: Vec<RuleRaw>) -> ProfileRaw {
    ProfileRaw {
        meta: ProfileMeta {
            name: "test".to_string(),
            description: Some("test profile".to_string()),
            format_version: 1,
        },
        rules,
    }
}

fn make_regex_rule(id: &str, regex: &str) -> RuleRaw {
    RuleRaw {
        id: id.to_string(),
        enabled: true,
        match_rule: MatchRule {
            host: None,
            path: None,
            not_path: None,
            method: None,
            regex: Some(regex.to_string()),
        },
        target: Target {
            host: "localhost".to_string(),
            port: 3000,
            scheme: "http".to_string(),
            path: None,
        },
        preserve_host: true,
        priority: 0,
    }
}

fn roundtrip(profile: &ProfileRaw) -> ProfileRaw {
    let toml_str = toml::to_string_pretty(profile).expect("serialize");
    toml::from_str(&toml_str).expect("deserialize")
}

// -- basic round-trip --

#[test]
fn roundtrip_profile_with_glob_rules() {
    let profile = make_profile(vec![RuleRaw {
        id: "glob_rule".to_string(),
        enabled: true,
        match_rule: MatchRule {
            host: Some("*.example.com".to_string()),
            path: Some("/api/*".to_string()),
            not_path: Some("/api/health".to_string()),
            method: Some("GET".to_string()),
            regex: None,
        },
        target: Target {
            host: "localhost".to_string(),
            port: 3000,
            scheme: "http".to_string(),
            path: None,
        },
        preserve_host: true,
        priority: 5,
    }]);

    let parsed = roundtrip(&profile);
    let r = &parsed.rules[0];
    assert_eq!(r.id, "glob_rule");
    assert_eq!(r.match_rule.host.as_deref(), Some("*.example.com"));
    assert_eq!(r.match_rule.path.as_deref(), Some("/api/*"));
    assert_eq!(r.match_rule.not_path.as_deref(), Some("/api/health"));
    assert_eq!(r.match_rule.method.as_deref(), Some("GET"));
    assert_eq!(r.target.port, 3000);
    assert_eq!(r.priority, 5);
}

// -- regex round-trips (the proxyman import case) --

#[test]
fn roundtrip_regex_with_backslash_dot() {
    // exact regex from the proxyman import that triggered the bug report
    let regex = r"^.*\.rechargeapps\.com/merchant(?!/v1).*";
    let profile = make_profile(vec![make_regex_rule("test", regex)]);
    let parsed = roundtrip(&profile);
    assert_eq!(parsed.rules[0].match_rule.regex.as_deref(), Some(regex));
}

#[test]
fn roundtrip_regex_with_lookahead() {
    let regex = r"^https://.*\.example\.com/merchant(?!/v1)(?!/health).*";
    let profile = make_profile(vec![make_regex_rule("test", regex)]);
    let parsed = roundtrip(&profile);
    assert_eq!(parsed.rules[0].match_rule.regex.as_deref(), Some(regex));
}

#[test]
fn roundtrip_regex_with_special_chars() {
    let regex = r#"^https://api\.(dev|staging)\.example\.com/v[0-9]+/.*\?token=.*"#;
    let profile = make_profile(vec![make_regex_rule("test", regex)]);
    let parsed = roundtrip(&profile);
    assert_eq!(parsed.rules[0].match_rule.regex.as_deref(), Some(regex));
}

#[test]
fn roundtrip_regex_with_backslashes_and_quotes() {
    let regex = r#"^https://.*\.example\.com/path/with\\slash"#;
    let profile = make_profile(vec![make_regex_rule("test", regex)]);
    let parsed = roundtrip(&profile);
    assert_eq!(parsed.rules[0].match_rule.regex.as_deref(), Some(regex));
}

// -- multi-rule profiles --

#[test]
fn roundtrip_multi_rule_profile() {
    let profile = make_profile(vec![
        make_regex_rule("rule_a", r"^.*\.example\.com/merchant(?!/v1).*"),
        make_regex_rule("rule_b", r"^.*\.example\.com/merchant/v1.*"),
        RuleRaw {
            id: "rule_c".to_string(),
            enabled: false,
            match_rule: MatchRule {
                host: Some("admin.example.com".to_string()),
                path: None,
                not_path: None,
                method: None,
                regex: None,
            },
            target: Target {
                host: "localhost".to_string(),
                port: 4000,
                scheme: "https".to_string(),
                path: Some("/admin".to_string()),
            },
            preserve_host: false,
            priority: 10,
        },
    ]);

    let parsed = roundtrip(&profile);
    assert_eq!(parsed.rules.len(), 3);
    assert_eq!(parsed.rules[0].id, "rule_a");
    assert_eq!(parsed.rules[1].id, "rule_b");
    assert_eq!(parsed.rules[2].id, "rule_c");
    assert!(!parsed.rules[2].enabled);
    assert_eq!(parsed.rules[2].target.scheme, "https");
    assert_eq!(parsed.rules[2].target.path.as_deref(), Some("/admin"));
    assert!(!parsed.rules[2].preserve_host);
}

// -- edge cases --

#[test]
fn roundtrip_empty_profile() {
    let profile = make_profile(vec![]);
    let parsed = roundtrip(&profile);
    assert!(parsed.rules.is_empty());
    assert_eq!(parsed.meta.name, "test");
}

#[test]
fn roundtrip_none_optional_fields() {
    // all optional match fields are None
    let profile = make_profile(vec![RuleRaw {
        id: "minimal".to_string(),
        enabled: true,
        match_rule: MatchRule {
            host: None,
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
    }]);
    let parsed = roundtrip(&profile);
    assert!(parsed.rules[0].match_rule.host.is_none());
    assert!(parsed.rules[0].match_rule.regex.is_none());
    assert!(parsed.rules[0].target.path.is_none());
}

// -- config round-trip --

#[test]
fn config_roundtrip() {
    use giantd::config::AppConfig;

    let config = AppConfig {
        version: 1,
        listen_port: 9456,
        pac_port: 9876,
        log_level: "debug".to_string(),
        log_max_size_mb: 20,
        log_max_files: 3,
        auto_start: true,
        default_profile: Some("preprod".to_string()),
        routing_mode: "pac".to_string(),
        browser: Some("firefox".to_string()),
        bypass_hosts: vec!["localhost".to_string(), "*.internal".to_string()],
    };

    let toml_str = toml::to_string_pretty(&config).expect("serialize config");
    let parsed: AppConfig = toml::from_str(&toml_str).expect("deserialize config");
    assert_eq!(parsed.listen_port, 9456);
    assert_eq!(parsed.pac_port, 9876);
    assert_eq!(parsed.log_level, "debug");
    assert!(parsed.auto_start);
    assert_eq!(parsed.default_profile, Some("preprod".to_string()));
    assert_eq!(parsed.routing_mode, "pac");
    assert_eq!(parsed.bypass_hosts.len(), 2);
}
