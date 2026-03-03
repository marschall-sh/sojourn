use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub settings: Settings,

    #[serde(default)]
    pub inventory: Vec<InventoryConfig>,

    #[serde(default)]
    pub ip_labels: Vec<IpLabel>,

    #[serde(default)]
    pub jump_hosts: Vec<JumpHostRule>,

    /// Per-host overrides set via the TUI editor (keyed by hostname)
    #[serde(default)]
    pub host_overrides: Vec<HostOverride>,
}

/// A user-defined override for a specific host, persisted in config.toml.
/// Overrides are matched by hostname and merged on top of inventory data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostOverride {
    pub hostname: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jump_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub default_user: Option<String>,
    pub ssh_extra_args: Option<String>,
    #[serde(default = "default_true")]
    pub connect_on_single_match: bool,
    /// Exit sojourn after the SSH session ends instead of returning to the TUI
    #[serde(default = "default_false")]
    pub exit_after_connect: bool,
    /// Name of the color theme: "default", "tokyo-night", "catppuccin", "dracula", "gruvbox", "nord"
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_theme() -> String {
    "default".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_user: None,
            ssh_extra_args: None,
            connect_on_single_match: true,
            exit_after_connect: false,
            theme: default_theme(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InventoryConfig {
    #[serde(rename = "ansible")]
    Ansible { path: String },
    #[serde(rename = "ssh_config")]
    SshConfig { path: String },
    #[serde(rename = "yaml")]
    Yaml { path: String },
    #[serde(rename = "shell_alias")]
    ShellAlias { path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpLabel {
    pub pattern: String,
    pub label: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JumpHostRule {
    pub host_pattern: String,
    pub jump_host: String,
    pub user: Option<String>,
}

impl Config {
    pub fn load(path: Option<&str>) -> Result<Self> {

        let config_path = if let Some(p) = path {
            Some(p.to_string())
        } else {
            find_default_config()
        };

        if let Some(path) = config_path {
            let content = fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("Failed to read config '{}': {}", path, e))?;
            let config: Config = toml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("Failed to parse config '{}': {}", path, e))?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    pub fn label_for_ip<'a>(&'a self, ip: &str) -> Option<&'a IpLabel> {
        self.ip_labels
            .iter()
            .find(|label| ip_matches_pattern(ip, &label.pattern))
    }

    /// Find the override entry for a given hostname, if any.
    #[allow(dead_code)]
    pub fn override_for(&self, hostname: &str) -> Option<&HostOverride> {
        self.host_overrides.iter().find(|o| o.hostname == hostname)
    }

    /// Upsert a host override — replaces existing entry or appends a new one.
    pub fn upsert_override(&mut self, ov: HostOverride) {
        if let Some(existing) = self.host_overrides.iter_mut().find(|o| o.hostname == ov.hostname) {
            *existing = ov;
        } else {
            self.host_overrides.push(ov);
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            inventory: vec![InventoryConfig::SshConfig {
                path: "~/.ssh/config".to_string(),
            }],
            ip_labels: vec![],
            jump_hosts: vec![],
            host_overrides: vec![],
        }
    }
}

fn find_default_config() -> Option<String> {
    let home = dirs::home_dir()?;
    let candidates = [
        home.join(".config/sojourn/config.toml"),
        home.join(".sojourn.toml"),
    ];
    candidates
        .iter()
        .find(|p| p.exists())
        .map(|p| p.display().to_string())
}

pub fn ip_matches_pattern(ip: &str, pattern: &str) -> bool {
    if pattern.contains('/') {
        if let (Ok(ip_addr), Ok(net)) = (
            ip.parse::<std::net::IpAddr>(),
            pattern.parse::<ipnet::IpNet>(),
        ) {
            return net.contains(&ip_addr);
        }
    }
    let prefix = pattern.trim_end_matches('*').trim_end_matches('.');
    ip.starts_with(prefix)
}
