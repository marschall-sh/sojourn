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
    // Line-by-line YAML parser for Ansible inventory files.
    //
    // Key design: we track `host_indent` — the indent level at which hostname
    // entries appear (e.g. 4 spaces under "all: > hosts:"). A bare `key:` is
    // only treated as a new hostname when its indent EQUALS host_indent.
    // Keys at deeper indentation (like `internal_ip:`, `external_ip:`) are
    // variables belonging to the current host and are simply ignored unless
    // they are ansible_host/user/port.

    const STRUCTURAL: &[&str] = &["all", "hosts", "children", "vars", "ungrouped"];

    let mut current_hostname: Option<String> = None;
    let mut current_ip:       Option<String> = None;
    let mut current_user:     Option<String> = None;
    let mut current_port:     Option<u16>    = None;
    let mut host_indent:      Option<usize>  = None; // locked-in hostname indent level

    macro_rules! flush {
        () => {
            if let Some(hostname) = current_hostname.take() {
                hosts.push(Host {
                    hostname,
                    ip:            current_ip.take(),
                    user:          current_user.take(),
                    port:          current_port.take(),
                    identity_file: None,
                    jump_host:     None,
                    groups:        vec![group.to_string()],
                    source:        source.to_string(),
                    tags:          std::collections::HashMap::new(),
                    label:         None,
                    alias:         None,
                });
            }
        };
    }

    for line in content.lines() {
        if line.trim().is_empty() || line.trim().starts_with('#') { continue; }
        let indent  = line.len() - line.trim_start().len();
        let trimmed = line.trim();

        // Skip document markers and pure structural keys.
        // `hosts:` is special: it introduces a new hostname block at an unknown
        // indent, so reset host_indent to let the next bare key establish it.
        if matches!(trimmed, "---" | "all:" | "ungrouped:" | "children:" | "vars:") {
            continue;
        }
        if trimmed == "hosts:" {
            host_indent = None; // entering a new hosts: block — re-learn indent
            continue;
        }

        let Some(colon) = trimmed.find(':') else { continue };
        let key = trimmed[..colon].trim();
        let val = trimmed[colon + 1..].trim();

        if val.is_empty() {
            // Bare `key:` — is this a hostname or an empty host variable?
            //
            // It's a hostname if:
            //   (a) we haven't locked in a host_indent yet, OR
            //   (b) this indent == host_indent (same level as other hostnames)
            //
            // It is NOT a hostname if indent > host_indent (it's a variable
            // like `external_ip:` that happens to have no value).
            let is_hostname_level = match host_indent {
                None     => true,
                Some(hi) => indent == hi,
            };

            if is_hostname_level
                && !STRUCTURAL.contains(&key)
                && key.chars().next().map(|c| c.is_alphanumeric() || c == '_' || c == '-').unwrap_or(false)
            {
                flush!();
                current_hostname = Some(key.to_string());
                host_indent      = Some(indent);
            }
            // else: empty variable for current host — ignore
        } else {
            // Key-value pair — only care about ansible_ vars
            match key {
                "ansible_host" => { current_ip   = Some(val.to_string()); }
                "ansible_user" => { current_user = Some(val.to_string()); }
                "ansible_port" => { current_port = val.parse().ok(); }
                _ => {}
            }
        }
    }

    flush!();
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
