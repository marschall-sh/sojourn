use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Host {
    pub hostname: String,
    pub ip: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub jump_host: Option<String>,
    pub groups: Vec<String>,
    pub source: String,
    pub tags: HashMap<String, String>,
    /// Custom label set by the user via the TUI editor
    pub label: Option<String>,
}

impl Host {
    /// Build a combined string used for fuzzy matching
    pub fn search_string(&self) -> String {
        let mut parts = vec![self.hostname.clone()];
        if let Some(ip) = &self.ip {
            parts.push(ip.clone());
        }
        if let Some(label) = &self.label {
            parts.push(label.clone());
        }
        for group in &self.groups {
            parts.push(group.clone());
        }
        for (k, v) in &self.tags {
            parts.push(format!("{}:{}", k, v));
        }
        parts.join(" ")
    }

    /// The actual target string passed to ssh (user@ip or user@hostname)
    pub fn connect_target(&self) -> String {
        let host = self.ip.as_deref().unwrap_or(&self.hostname);
        match &self.user {
            Some(u) => format!("{}@{}", u, host),
            None => host.to_string(),
        }
    }
}

pub trait InventorySource {
    fn load(&self) -> anyhow::Result<Vec<Host>>;
}

pub mod ansible;
pub mod shell_alias;
pub mod ssh_config;
pub mod yaml_config;
