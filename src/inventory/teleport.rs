use super::{Host, InventorySource};
use anyhow::{bail, Result};
use std::collections::HashMap;
use std::process::Command;

pub struct TeleportInventory {
    pub tsh_binary: String,
    pub proxy: Option<String>,
}

/// Finds the tsh binary, checking common install locations before $PATH.
pub fn find_tsh_binary() -> Option<String> {
    let candidates = [
        "/usr/local/bin/tsh",
        "/usr/bin/tsh",
        "/opt/homebrew/bin/tsh",
        "/opt/homebrew/sbin/tsh",
        "/home/linuxbrew/.linuxbrew/bin/tsh",
    ];

    for path in &candidates {
        if std::path::Path::new(path).exists() {
            return Some(path.to_string());
        }
    }

    // Fall back to PATH lookup
    which_tsh()
}

fn which_tsh() -> Option<String> {
    Command::new("which")
        .arg("tsh")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

impl TeleportInventory {
    fn tsh_args(&self, subcommand: &str) -> Vec<String> {
        let mut args = vec![subcommand.to_string()];
        if let Some(proxy) = &self.proxy {
            args.push(format!("--proxy={}", proxy));
        }
        args
    }

    /// Check whether the current tsh session is valid.
    /// Returns Ok(true) if logged in, Ok(false) if expired/missing.
    pub fn is_logged_in(&self) -> bool {
        let mut args = self.tsh_args("status");
        args.push("--format=json".to_string());
        Command::new(&self.tsh_binary)
            .args(&args)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

impl InventorySource for TeleportInventory {
    fn load(&self) -> Result<Vec<Host>> {
        let mut args = self.tsh_args("ls");
        args.push("--format=json".to_string());

        let output = Command::new(&self.tsh_binary)
            .args(&args)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("tsh ls failed: {}", stderr.trim());
        }

        let json = String::from_utf8_lossy(&output.stdout);
        parse_tsh_ls(&json)
    }
}

fn parse_tsh_ls(json: &str) -> Result<Vec<Host>> {
    let nodes: Vec<serde_json::Value> = serde_json::from_str(json.trim())?;
    let mut hosts = Vec::new();

    for node in nodes {
        let hostname = node["spec"]["hostname"]
            .as_str()
            .or_else(|| node["metadata"]["name"].as_str())
            .unwrap_or("")
            .to_string();

        if hostname.is_empty() {
            continue;
        }

        let addr = node["spec"]["addr"].as_str().unwrap_or("").to_string();
        let ip = if addr.is_empty() {
            None
        } else {
            Some(addr.split(':').next().unwrap_or(&addr).to_string())
        };

        let mut tags: HashMap<String, String> = HashMap::new();
        if let Some(label_map) = node["metadata"]["labels"].as_object() {
            for (k, v) in label_map {
                if let Some(val) = v.as_str() {
                    tags.insert(k.clone(), val.to_string());
                }
            }
        }

        hosts.push(Host {
            hostname: hostname.clone(),
            ip,
            user: None,
            port: None,
            identity_file: None,
            jump_host: None,
            groups: vec![],
            source: "teleport".to_string(),
            tags,
            label: None,
            alias: None,
        });
    }

    Ok(hosts)
}
