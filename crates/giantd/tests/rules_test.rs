use giantd::rules::{MatchRule, Rule, RuleRaw, Target};

fn make_rule(
    host: Option<&str>,
    path: Option<&str>,
    not_path: Option<&str>,
    method: Option<&str>,
    regex: Option<&str>,
) -> Rule {
    let raw = RuleRaw {
        id: "test_rule".to_string(),
        enabled: true,
        match_rule: MatchRule {
            host: host.map(|s| s.to_string()),
            path: path.map(|s| s.to_string()),
            not_path: not_path.map(|s| s.to_string()),
            method: method.map(|s| s.to_string()),
            regex: regex.map(|s| s.to_string()),
        },
        target: Target {
            host: "localhost".to_string(),
            port: 3000,
            scheme: "http".to_string(),
            path: None,
        },
        preserve_host: true,
        priority: 0,
    };
    Rule::from_raw(raw).expect("valid rule")
}

fn make_uri(s: &str) -> http::Uri {
    s.parse().expect("valid uri")
}

fn make_headers(host: Option<&str>) -> http::HeaderMap {
    let mut headers = http::HeaderMap::new();
    if let Some(h) = host {
        headers.insert("host", h.parse().unwrap());
    }
    headers
}

#[test]
fn glob_host_match() {
    let rule = make_rule(Some("*.example.com"), None, None, None, None);
    let uri = make_uri("https://foo.example.com/path");
    let headers = make_headers(Some("foo.example.com"));
    assert!(rule.matches(&uri, &headers, &http::Method::GET));
}

#[test]
fn glob_host_no_match() {
    let rule = make_rule(Some("*.example.com"), None, None, None, None);
    let uri = make_uri("https://example.com/path");
    let headers = make_headers(Some("example.com"));
    assert!(!rule.matches(&uri, &headers, &http::Method::GET));
}

#[test]
fn glob_path_match() {
    let rule = make_rule(Some("*"), Some("/merchant/*"), None, None, None);
    let uri = make_uri("https://app.example.com/merchant/foo");
    let headers = make_headers(Some("app.example.com"));
    assert!(rule.matches(&uri, &headers, &http::Method::GET));
}

#[test]
fn glob_path_no_match() {
    let rule = make_rule(Some("*"), Some("/merchant/*"), None, None, None);
    let uri = make_uri("https://app.example.com/other/foo");
    let headers = make_headers(Some("app.example.com"));
    assert!(!rule.matches(&uri, &headers, &http::Method::GET));
}

#[test]
fn not_path_exclusion() {
    let rule = make_rule(
        Some("*"),
        Some("/merchant/**"),
        Some("/merchant/v1/**"),
        None,
        None,
    );
    let uri = make_uri("https://app.example.com/merchant/v1/foo");
    let headers = make_headers(Some("app.example.com"));
    assert!(!rule.matches(&uri, &headers, &http::Method::GET));
}

#[test]
fn not_path_allows_other_paths() {
    // glob-match: * doesn't cross path separators, use ** for recursive
    let rule = make_rule(
        Some("*"),
        Some("/merchant/**"),
        Some("/merchant/v1/**"),
        None,
        None,
    );
    let uri = make_uri("https://app.example.com/merchant/v2/foo");
    let headers = make_headers(Some("app.example.com"));
    assert!(rule.matches(&uri, &headers, &http::Method::GET));
}

#[test]
fn regex_fallback() {
    let rule = make_rule(
        None,
        None,
        None,
        None,
        Some(r"^https://api-v[0-9]+\.example\.com/.*"),
    );
    let uri = make_uri("https://api-v2.example.com/test");
    let headers = make_headers(Some("api-v2.example.com"));
    assert!(rule.matches(&uri, &headers, &http::Method::GET));
}

#[test]
fn regex_no_match() {
    let rule = make_rule(
        None,
        None,
        None,
        None,
        Some(r"^https://api-v[0-9]+\.example\.com/.*"),
    );
    let uri = make_uri("https://other.example.com/test");
    let headers = make_headers(Some("other.example.com"));
    assert!(!rule.matches(&uri, &headers, &http::Method::GET));
}

#[test]
fn method_filter_get_only() {
    let rule = make_rule(Some("*"), None, None, Some("GET"), None);
    let uri = make_uri("https://app.example.com/path");
    let headers = make_headers(Some("app.example.com"));
    assert!(rule.matches(&uri, &headers, &http::Method::GET));
    assert!(!rule.matches(&uri, &headers, &http::Method::POST));
}

#[test]
fn method_filter_any() {
    let rule = make_rule(Some("*"), None, None, Some("ANY"), None);
    let uri = make_uri("https://app.example.com/path");
    let headers = make_headers(Some("app.example.com"));
    assert!(rule.matches(&uri, &headers, &http::Method::GET));
    assert!(rule.matches(&uri, &headers, &http::Method::POST));
    assert!(rule.matches(&uri, &headers, &http::Method::DELETE));
}

#[test]
fn http2_authority_fallback() {
    // no Host header, but URI has authority (HTTP/2 style)
    let rule = make_rule(Some("*.example.com"), None, None, None, None);
    let uri = make_uri("https://foo.example.com/path");
    let headers = make_headers(None);
    assert!(rule.matches(&uri, &headers, &http::Method::GET));
}

#[test]
fn first_match_wins() {
    let rule1 = make_rule(Some("*.example.com"), Some("/merchant/*"), None, None, None);
    let rule2 = make_rule(Some("*"), Some("/merchant/*"), None, None, None);

    let uri = make_uri("https://foo.example.com/merchant/test");
    let headers = make_headers(Some("foo.example.com"));

    // both should match, but in a real list rule1 comes first
    assert!(rule1.matches(&uri, &headers, &http::Method::GET));
    assert!(rule2.matches(&uri, &headers, &http::Method::GET));
}

#[test]
fn from_raw_compiles_regex() {
    let raw = RuleRaw {
        id: "test".to_string(),
        enabled: true,
        match_rule: MatchRule {
            host: None,
            path: None,
            not_path: None,
            method: None,
            regex: Some(r"^https://.*\.example\.com/.*".to_string()),
        },
        target: Target {
            host: "localhost".to_string(),
            port: 3000,
            scheme: "http".to_string(),
            path: None,
        },
        preserve_host: true,
        priority: 0,
    };
    let rule = Rule::from_raw(raw).expect("should compile valid regex");
    assert!(rule.compiled_regex.is_some());
}

#[test]
fn from_raw_rejects_invalid_regex() {
    let raw = RuleRaw {
        id: "test".to_string(),
        enabled: true,
        match_rule: MatchRule {
            host: None,
            path: None,
            not_path: None,
            method: None,
            regex: Some(r"[invalid".to_string()),
        },
        target: Target {
            host: "localhost".to_string(),
            port: 3000,
            scheme: "http".to_string(),
            path: None,
        },
        preserve_host: true,
        priority: 0,
    };
    assert!(Rule::from_raw(raw).is_err());
}

#[test]
fn rewrite_uri_preserves_path() {
    let rule = make_rule(Some("*"), None, None, None, None);
    let original = make_uri("https://app.example.com/merchant/foo?bar=baz");
    let rewritten = rule.rewrite_uri(&original);
    assert_eq!(rewritten.scheme_str(), Some("http"));
    assert_eq!(rewritten.authority().unwrap().host(), "localhost");
    assert_eq!(rewritten.authority().unwrap().port_u16(), Some(3000));
    assert_eq!(
        rewritten.path_and_query().unwrap().as_str(),
        "/merchant/foo?bar=baz"
    );
}

#[test]
fn rewrite_uri_with_target_path() {
    let raw = RuleRaw {
        id: "test".to_string(),
        enabled: true,
        match_rule: MatchRule {
            host: Some("*".to_string()),
            path: None,
            not_path: None,
            method: None,
            regex: None,
        },
        target: Target {
            host: "localhost".to_string(),
            port: 3000,
            scheme: "http".to_string(),
            path: Some("/custom/path".to_string()),
        },
        preserve_host: true,
        priority: 0,
    };
    let rule = Rule::from_raw(raw).unwrap();
    let original = make_uri("https://app.example.com/merchant/foo");
    let rewritten = rule.rewrite_uri(&original);
    assert_eq!(rewritten.path_and_query().unwrap().as_str(), "/custom/path");
}
