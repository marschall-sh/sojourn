use super::{Host, InventorySource};
use anyhow::Result;
use std::collections::HashMap;
use std::fs;

pub struct SshConfigInventory {
    pub path: String,
}

impl InventorySource for SshConfigInventory {
    fn load(&self) -> Result<Vec<Host>> {
        let expanded = shellexpand::tilde(&self.path).to_string();
        let content = match fs::read_to_string(&expanded) {
            Ok(c) => c,
            Err(_) => return Ok(vec![]),
        };

        let mut hosts: Vec<Host> = Vec::new();
        let mut current: Option<Host> = None;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = split_ssh_config_line(line) {
                match key.to_lowercase().as_str() {
                    "host" => {
                        if let Some(h) = current.take() {
                            if !h.hostname.contains('*') && !h.hostname.contains('?') {
                                hosts.push(h);
                            }
                        }
                        current = Some(Host {
                            hostname: value.to_string(),
                            ip: None,
                            user: None,
                            port: None,
                            identity_file: None,
                            jump_host: None,
                            groups: vec!["ssh_config".to_string()],
                            source: self.path.clone(),
                            tags: HashMap::new(),
                            label: None,
                            alias: None,
                        });
                    }
                    "hostname" => {
                        if let Some(h) = current.as_mut() {
                            h.ip = Some(value.to_string());
                        }
                    }
                    "user" => {
                        if let Some(h) = current.as_mut() {
                            h.user = Some(value.to_string());
                        }
                    }
                    "port" => {
                        if let Some(h) = current.as_mut() {
                            h.port = value.parse().ok();
                        }
                    }
                    "identityfile" => {
                        if let Some(h) = current.as_mut() {
                            h.identity_file = Some(value.to_string());
                        }
                    }
                    "proxyjump" => {
                        if let Some(h) = current.as_mut() {
                            h.jump_host = Some(value.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Some(h) = current.take() {
            if !h.hostname.contains('*') && !h.hostname.contains('?') {
                hosts.push(h);
            }
        }

        Ok(hosts)
    }
}

fn split_ssh_config_line(line: &str) -> Option<(&str, &str)> {
    // SSH config uses either "Key Value" or "Key=Value"
    if let Some(pos) = line.find('=') {
        let key = line[..pos].trim();
        let value = line[pos + 1..].trim();
        if !key.is_empty() && !value.is_empty() {
            return Some((key, value));
        }
    }
    if let Some(pos) = line.find(char::is_whitespace) {
        let key = line[..pos].trim();
        let value = line[pos..].trim();
        if !key.is_empty() && !value.is_empty() {
            return Some((key, value));
        }
    }
    None
}
