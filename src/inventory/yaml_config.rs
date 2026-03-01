use super::{Host, InventorySource};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Deserialize, Serialize)]
pub struct YamlHost {
    pub hostname: String,
    pub ip: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub jump_host: Option<String>,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct YamlConfig {
    #[serde(default)]
    pub hosts: Vec<YamlHost>,
}

pub struct YamlInventory {
    pub path: String,
}

impl InventorySource for YamlInventory {
    fn load(&self) -> Result<Vec<Host>> {
        let expanded = shellexpand::tilde(&self.path).to_string();
        let content = match fs::read_to_string(&expanded) {
            Ok(c) => c,
            Err(_) => return Ok(vec![]),
        };

        let yaml_config: YamlConfig = serde_yaml::from_str(&content)?;

        Ok(yaml_config
            .hosts
            .into_iter()
            .map(|h| Host {
                hostname: h.hostname,
                ip: h.ip,
                user: h.user,
                port: h.port,
                identity_file: h.identity_file,
                jump_host: h.jump_host,
                groups: h.groups,
                source: expanded.clone(),
                tags: h.tags,
                label: None,
                alias: None,
            })
            .collect())
    }
}
