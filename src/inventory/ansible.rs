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

            let group = extract_group_from_path(&path);
            let source = path.display().to_string();

            let content = match fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
                    continue;
                }

                if let Some(host) = parse_ansible_host_line(line, &group, &source) {
                    hosts.push(host);
                }
            }
        }

        Ok(hosts)
    }
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
    if parts.is_empty() {
        return None;
    }

    let hostname = parts[0].to_string();

    let ip = extract_var(line, "ansible_host");
    let user = extract_var(line, "ansible_user");
    let port: Option<u16> = extract_var(line, "ansible_port")
        .and_then(|p| p.parse().ok());

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
