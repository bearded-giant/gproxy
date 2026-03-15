use crate::rules::Rule;

pub fn generate_pac(rules: &[Rule], proxy_port: u16) -> String {
    let mut clauses = Vec::new();
    let mut seen_hosts = std::collections::HashSet::new();

    for rule in rules.iter().filter(|r| r.enabled) {
        if let Some(ref host) = rule.match_rule.host {
            if seen_hosts.insert(host.clone()) {
                // convert glob pattern to shExpMatch pattern
                clauses.push(format!(
                    "  if (shExpMatch(host, \"{}\")) {{\n    return \"PROXY 127.0.0.1:{}\";\n  }}",
                    host, proxy_port
                ));
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
