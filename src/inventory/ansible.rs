use super::{Host, InventorySource};
use anyhow::Result;
use glob::glob;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub struct AnsibleInventory {
    pub pattern: String,
}

impl InventorySource for AnsibleInventory {
    fn load(&self) -> Result<Vec<Host>> {
        let expanded = shellexpand::tilde(&self.pattern).to_string();
        let mut hosts = Vec::new();

        for entry in glob(&expanded)? {
            let path = match entry {
                Ok(p) => p,
                Err(_) => continue,
            };

            // Directory-based inventory: walk one level for host files
            if path.is_dir() {
                if let Ok(entries) = fs::read_dir(&path) {
                    for sub in entries.flatten() {
                        let sub_path = sub.path();
                        if sub_path.is_file() {
                            let source = sub_path.display().to_string();
                            let group = extract_group_from_path(&sub_path);
                            if let Ok(content) = fs::read_to_string(&sub_path) {
                                parse_inventory_content(&content, &group, &source, &mut hosts);
                            }
                        }
                    }
                }
                continue;
            }

            let group = extract_group_from_path(&path);
            let source = path.display().to_string();

            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            parse_inventory_content(&content, &group, &source, &mut hosts);
        }

        Ok(hosts)
    }
}

fn parse_inventory_content(content: &str, group: &str, source: &str, hosts: &mut Vec<Host>) {
    // Detect YAML inventory (starts with --- or has 'all:' / 'hosts:' keys)
    let trimmed = content.trim_start();
    if trimmed.starts_with("---")
        || trimmed.starts_with("all:")
        || trimmed.contains("\nhosts:")
        || trimmed.contains("ansible_host:")
    {
        parse_yaml_ansible_inventory(content, group, source, hosts);
    } else {
        parse_ini_ansible_inventory(content, group, source, hosts);
    }
}

/// Parse INI-style Ansible inventory (classic hosts_* format)
fn parse_ini_ansible_inventory(content: &str, group: &str, source: &str, hosts: &mut Vec<Host>) {
    let mut current_group = group.to_string();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        // Group header [groupname] or [groupname:vars] / [groupname:children]
        if line.starts_with('[') {
            let inner = line.trim_matches(|c| c == '[' || c == ']');
            // Skip :vars and :children sections
            if inner.contains(':') {
                current_group = inner.split(':').next().unwrap_or(group).to_string();
            } else {
                current_group = inner.to_string();
            }
            continue;
        }
        if let Some(host) = parse_ansible_host_line(line, &current_group, source) {
            hosts.push(host);
        }
    }
}

/// Parse YAML-style Ansible inventory
/// Supports:
///   all:
///     hosts:
///       web01:
///         ansible_host: 10.0.0.1
///         ansible_user: deploy
fn parse_yaml_ansible_inventory(content: &str, group: &str, source: &str, hosts: &mut Vec<Host>) {
    // Simple line-by-line parser — covers the common flat structure
    // without pulling in a full YAML dependency for this path
    let mut current_hostname: Option<String> = None;
    let mut current_ip: Option<String> = None;
    let mut current_user: Option<String> = None;
    let mut current_port: Option<u16> = None;
    let mut current_indent: usize = 0;

    let flush = |hn: &mut Option<String>, ip: &mut Option<String>,
                  user: &mut Option<String>, port: &mut Option<u16>,
                  hosts: &mut Vec<Host>| {
        if let Some(hostname) = hn.take() {
            hosts.push(Host {
                hostname,
                ip: ip.take(),
                user: user.take(),
                port: port.take(),
                identity_file: None,
                jump_host: None,
                groups: vec![group.to_string()],
                source: source.to_string(),
                tags: std::collections::HashMap::new(),
                label: None,
                alias: None,
            });
        }
        *ip = None; *user = None; *port = None;
    };

    for line in content.lines() {
        if line.trim().is_empty() || line.trim().starts_with('#') { continue; }
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();

        // Skip YAML document markers and top-level structural keys
        if trimmed == "---" || trimmed == "all:" || trimmed == "hosts:"
            || trimmed.ends_with(':') && !trimmed.contains(' ')
            && ["ungrouped:", "children:", "vars:"].contains(&trimmed) {
            continue;
        }

        // A key: value pair
        if let Some(colon) = trimmed.find(':') {
            let key = trimmed[..colon].trim();
            let val = trimmed[colon + 1..].trim();

            match key {
                "ansible_host" if !val.is_empty() => { current_ip = Some(val.to_string()); }
                "ansible_user" if !val.is_empty() => { current_user = Some(val.to_string()); }
                "ansible_port" if !val.is_empty() => { current_port = val.parse().ok(); }
                _ if val.is_empty() => {
                    // A bare key: with no value — could be a hostname entry
                    // Only treat as hostname if it's at a deeper indent than "hosts:"
                    // and doesn't look like a structural key
                    if indent > current_indent || current_hostname.is_none() {
                        flush(&mut current_hostname, &mut current_ip,
                              &mut current_user, &mut current_port, hosts);
                        // Heuristic: if key looks like a hostname (has dot or alphanum)
                        if !["all", "hosts", "children", "vars", "ungrouped"].contains(&key)
                            && key.chars().next().map(|c| c.is_alphanumeric()).unwrap_or(false)
                        {
                            current_hostname = Some(key.to_string());
                            current_indent = indent;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    flush(&mut current_hostname, &mut current_ip,
          &mut current_user, &mut current_port, hosts);
}

fn extract_group_from_path(path: &Path) -> String {
    path.parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("ansible")
        .to_string()
}

fn parse_ansible_host_line(line: &str, group: &str, source: &str) -> Option<Host> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() { return None; }

    let raw_host = parts[0];

    // Skip :vars / :children section content (key=value without a hostname)
    if raw_host.contains('=') { return None; }

    // hostname may be host:port — split it
    let (hostname, inline_port) = if let Some(colon) = raw_host.rfind(':') {
        let port_str = &raw_host[colon + 1..];
        if port_str.chars().all(|c| c.is_ascii_digit()) {
            (raw_host[..colon].to_string(), port_str.parse::<u16>().ok())
        } else {
            (raw_host.to_string(), None)
        }
    } else {
        (raw_host.to_string(), None)
    };

    let ip   = extract_var(line, "ansible_host");
    let user = extract_var(line, "ansible_user");
    let port = extract_var(line, "ansible_port")
        .and_then(|p| p.parse().ok())
        .or(inline_port);

    Some(Host {
        hostname,
        ip,
        user,
        port,
        identity_file: None,
        jump_host: None,
        groups: vec![group.to_string()],
        source: source.to_string(),
        tags: HashMap::new(),
        label: None,
        alias: None,
    })
}

fn extract_var(line: &str, key: &str) -> Option<String> {
    let pattern = format!(r"{}=(\S+)", regex::escape(key));
    let re = Regex::new(&pattern).ok()?;
    re.captures(line)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}
