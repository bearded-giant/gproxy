use crate::rules::Rule;

pub fn generate_pac(rules: &[Rule], proxy_port: u16) -> String {
    let mut clauses = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for rule in rules.iter().filter(|r| r.enabled) {
        // extract host patterns from glob-based rules
        if let Some(ref host) = rule.match_rule.host {
            if seen.insert(host.clone()) {
                clauses.push(format!(
                    "  if (shExpMatch(host, \"{}\")) {{\n    return \"PROXY 127.0.0.1:{}\";\n  }}",
                    host, proxy_port
                ));
            }
        }

        // extract host patterns from regex-based rules
        if let Some(ref re) = rule.match_rule.regex {
            for host in extract_hosts_from_regex(re) {
                if seen.insert(host.clone()) {
                    clauses.push(format!(
                        "  if (shExpMatch(host, \"{}\")) {{\n    return \"PROXY 127.0.0.1:{}\";\n  }}",
                        host, proxy_port
                    ));
                }
            }
        }
    }

    let body = if clauses.is_empty() {
        "  return \"DIRECT\";".to_string()
    } else {
        format!("{}\n  return \"DIRECT\";", clauses.join("\n"))
    };

    format!("function FindProxyForURL(url, host) {{\n{}\n}}", body)
}

// pull host glob patterns from common regex forms like:
// ^(https)://.*\.example\.com/path(?!/excluded).*
fn extract_hosts_from_regex(re: &str) -> Vec<String> {
    let mut hosts = Vec::new();

    // strip leading ^(https|http):// prefix
    let rest = re
        .trim_start_matches('^')
        .trim_start_matches("(https)")
        .trim_start_matches("(http)")
        .trim_start_matches("https")
        .trim_start_matches("http")
        .trim_start_matches("://");

    // skip leading .* wildcard
    let rest = rest.trim_start_matches(".*");

    // extract the literal host part: \.foo\.bar\.com up to the next /
    if let Some(slash_pos) = rest.find('/') {
        let host_part = &rest[..slash_pos];
        // convert regex escapes to literal: \. -> .
        let host = host_part.replace("\\.", ".");
        // only use if it looks like a real hostname (has dots, no regex metacharacters)
        if host.contains('.') && !host.contains('(') && !host.contains('[') {
            // prepend * for the wildcard prefix that .* matched
            hosts.push(format!("*{}", host));
        }
    }

    hosts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_host_from_regex_with_lookahead() {
        let hosts =
            extract_hosts_from_regex(r"^(https)://.*\.preprod\.example\.com/merchant(?!/v1).*");
        assert_eq!(hosts, vec!["*.preprod.example.com"]);
    }

    #[test]
    fn extracts_host_from_simple_regex() {
        let hosts = extract_hosts_from_regex(r"^(https)://.*\.stage\.example\.com/tools(?!/v1).*");
        assert_eq!(hosts, vec!["*.stage.example.com"]);
    }

    #[test]
    fn no_host_from_garbage() {
        let hosts = extract_hosts_from_regex(r"^foobar");
        assert!(hosts.is_empty());
    }

    #[test]
    fn pac_output_uses_direct_default() {
        let pac = generate_pac(&[], 9456);
        assert!(pac.contains("DIRECT"));
        assert!(!pac.contains("PROXY"));
    }
}
