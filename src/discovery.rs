use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use walkdir::WalkDir;

use crate::config::InventoryConfig;
use crate::inventory::{
    ansible::AnsibleInventory,
    shell_alias::{shell_rc_files, ShellAliasInventory},
    ssh_config::SshConfigInventory,
    yaml_config::YamlInventory,
    InventorySource,
};

/// A source found during discovery, ready to be added to config
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct DiscoveredSource {
    pub config: InventoryConfig,
    pub label: String,        // human-readable description shown in wizard
    pub host_count: usize,    // reserved for future use (e.g. display in wizard)
    pub selected: bool,       // whether the user has ticked it in the wizard
}

/// Directories to always skip during scanning (performance + correctness)
const SKIP_DIRS: &[&str] = &[
    "node_modules", ".git", "target", "vendor", "__pycache__",
    "dist", "build", ".cache", ".npm", ".cargo", "venv", ".venv",
    ".tox", "coverage", ".idea", ".vscode", "roles", "collections",
];

/// Maximum scan depth below a root search path
const MAX_DEPTH: usize = 6;

/// How long to scan before giving up and returning partial results
pub const SCAN_TIMEOUT: Duration = Duration::from_secs(8);

// ─────────────────────────────────────────────────────────────────────────────

/// Run the full discovery pipeline. Returns results as they are found via the
/// returned channel so the TUI can show live progress.
pub fn discover_async() -> mpsc::Receiver<DiscoveredSource> {
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let deadline = Instant::now() + SCAN_TIMEOUT;
        let mut seen_paths: HashSet<PathBuf> = HashSet::new();

        // ── 0. Shell RC files (.zshrc, .bashrc, …) ───────────────────────────
        for rc_path in shell_rc_files() {
            let path_str = rc_path.display().to_string();
            let inv = ShellAliasInventory { path: path_str.clone() };
            if let Ok(hosts) = inv.load() {
                if !hosts.is_empty() {
                    seen_paths.insert(rc_path.clone());
                    let count = hosts.len();
                    let label = format!(
                        "{}  ({} ssh aliases)",
                        shorten(&path_str, 52),
                        count
                    );
                    let _ = tx.send(DiscoveredSource {
                        config: InventoryConfig::ShellAlias { path: path_str },
                        label,
                        host_count: count,
                        selected: true,
                    });
                }
            }
        }

        // ── 1. SSH config ────────────────────────────────────────────────────
        if let Some(home) = dirs::home_dir() {
            let ssh_cfg = home.join(".ssh/config");
            if ssh_cfg.exists() {
                if let Some(src) = probe_ssh_config(&ssh_cfg) {
                    seen_paths.insert(ssh_cfg);
                    let _ = tx.send(src);
                }
            }
        }

        // ── 2. $ANSIBLE_INVENTORY env var ────────────────────────────────────
        if let Ok(inv) = std::env::var("ANSIBLE_INVENTORY") {
            let p = PathBuf::from(shellexpand::tilde(&inv).as_ref());
            if p.exists() && !seen_paths.contains(&p) {
                if let Some(src) = probe_path(&p) {
                    seen_paths.insert(p);
                    let _ = tx.send(src);
                }
            }
        }

        // ── 3. ansible.cfg files (most reliable: reads the actual inventory= key) ──
        let scan_roots_early = repo_scan_roots();
        for root in &scan_roots_early {
            if !root.exists() { continue; }
            // Walk shallowly for ansible.cfg (depth 3 is enough for repo roots)
            let walker = WalkDir::new(root)
                .max_depth(3)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| !should_skip(e.path()));
            for entry in walker.flatten() {
                if Instant::now() >= deadline { break; }
                let path = entry.path();
                if path.file_name().and_then(|n| n.to_str()) == Some("ansible.cfg") {
                    for src in probe_ansible_cfg(path, &seen_paths) {
                        let key = src_path_key(&src);
                        if !seen_paths.contains(&key) {
                            seen_paths.insert(key);
                            let _ = tx.send(src);
                        }
                    }
                }
            }
        }

        // ── 4. High-probability fixed glob patterns (fast) ───────────────────
        let candidates = high_probability_paths();
        for pattern in &candidates {
            if Instant::now() >= deadline { break; }
            let expanded = shellexpand::tilde(pattern).to_string();
            for path in glob::glob(&expanded).into_iter().flatten().flatten() {
                if Instant::now() >= deadline { break; }
                let canonical = path.canonicalize().unwrap_or(path.clone());
                if seen_paths.contains(&canonical) { continue; }
                if let Some(src) = probe_path(&path) {
                    seen_paths.insert(canonical);
                    let _ = tx.send(src);
                }
            }
        }

        // ── 5. Deep repo scan (parallel, depth-limited) ──────────────────────
        let scan_roots = repo_scan_roots();
        let (deep_tx, deep_rx) = mpsc::channel::<DiscoveredSource>();
        let seen_snap = seen_paths.clone();
        let roots = scan_roots.clone();
        let dtx = deep_tx.clone();

        std::thread::spawn(move || {
            use rayon::prelude::*;
            roots.par_iter().for_each(|root| {
                if !root.exists() { return; }
                let walker = WalkDir::new(root)
                    .max_depth(MAX_DEPTH)
                    .follow_links(false)
                    .into_iter()
                    .filter_entry(|e| !should_skip(e.path()));

                for entry in walker.flatten() {
                    let path = entry.path();
                    if is_ansible_inventory_file(path) || is_yaml_inventory(path) {
                        let canonical = path.canonicalize().unwrap_or(path.to_path_buf());
                        if seen_snap.contains(&canonical) { continue; }
                        if let Some(src) = probe_path(path) {
                            let _ = dtx.send(src);
                        }
                    }
                }
            });
        });

        drop(deep_tx);

        // Drain deep scan results until timeout
        loop {
            if Instant::now() >= deadline { break; }
            match deep_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(src) => {
                    let key = src_path_key(&src);
                    let canonical = key.canonicalize().unwrap_or(key);
                    if !seen_paths.contains(&canonical) {
                        seen_paths.insert(canonical);
                        let _ = tx.send(src);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
            }
        }
    });

    rx
}

// ─────────────────────────────────────────────────────────────────────────────
// ansible.cfg parser
// ─────────────────────────────────────────────────────────────────────────────

/// Parse an ansible.cfg file and probe whatever `inventory =` points at.
/// Returns 0–N discovered sources (the inventory= value may itself be a glob).
fn probe_ansible_cfg(cfg_path: &Path, seen: &HashSet<PathBuf>) -> Vec<DiscoveredSource> {
    let Ok(contents) = std::fs::read_to_string(cfg_path) else {
        return vec![];
    };

    let mut in_defaults = false;
    let mut inventory_val: Option<String> = None;

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_defaults = trimmed.eq_ignore_ascii_case("[defaults]");
            continue;
        }
        if in_defaults {
            if let Some(rest) = trimmed.strip_prefix("inventory") {
                let rest = rest.trim_start();
                if let Some(rest) = rest.strip_prefix('=') {
                    let val = rest.trim().to_string();
                    if !val.is_empty() {
                        inventory_val = Some(val);
                        break;
                    }
                }
            }
        }
    }

    let Some(raw) = inventory_val else { return vec![]; };

    // The path may be relative to the cfg file's directory
    let cfg_dir = cfg_path.parent().unwrap_or(Path::new("."));
    let expanded = shellexpand::tilde(&raw).to_string();
    let resolved = if Path::new(&expanded).is_absolute() {
        PathBuf::from(&expanded)
    } else {
        cfg_dir.join(&expanded)
    };

    let mut results = vec![];
    for path in glob::glob(&resolved.display().to_string())
        .into_iter()
        .flatten()
        .flatten()
    {
        let canonical = path.canonicalize().unwrap_or(path.clone());
        if seen.contains(&canonical) { continue; }
        if let Some(src) = probe_path(&path) {
            results.push(src);
        }
    }
    results
}

// ─────────────────────────────────────────────────────────────────────────────
// Probing helpers
// ─────────────────────────────────────────────────────────────────────────────

fn probe_ssh_config(path: &Path) -> Option<DiscoveredSource> {
    let inv = SshConfigInventory {
        path: path.display().to_string(),
    };
    let hosts = inv.load().unwrap_or_default();
    let count = hosts.len();
    if count == 0 { return None; }
    Some(DiscoveredSource {
        config: InventoryConfig::SshConfig { path: path.display().to_string() },
        label: format!("~/.ssh/config  ({} hosts)", count),
        host_count: count,
        selected: true,
    })
}

fn probe_path(path: &Path) -> Option<DiscoveredSource> {
    let path_str = path.display().to_string();

    if is_ansible_inventory_file(path) {
        // If it's a hosts_* variant, glob siblings; otherwise use the file directly
        let pattern = if path.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with("hosts_") || n == "hosts")
            .unwrap_or(false)
        {
            parent_glob_pattern(path)
        } else {
            path_str.clone()
        };
        let inv = AnsibleInventory { pattern: pattern.clone() };
        let hosts = inv.load().unwrap_or_default();
        let count = hosts.len();
        if count == 0 { return None; }
        let label = format!("{}  ({} hosts, ansible)", shorten(&pattern, 52), count);
        return Some(DiscoveredSource {
            config: InventoryConfig::Ansible { path: pattern },
            label,
            host_count: count,
            selected: true,
        });
    }

    if is_yaml_inventory(path) {
        let inv = YamlInventory { path: path_str.clone() };
        let hosts = inv.load().unwrap_or_default();
        let count = hosts.len();
        if count == 0 { return None; }
        let label = format!("{}  ({} hosts, yaml)", shorten(&path_str, 52), count);
        return Some(DiscoveredSource {
            config: InventoryConfig::Yaml { path: path_str },
            label,
            host_count: count,
            selected: true,
        });
    }

    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Path detection helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Turn  /home/user/repos/ansible/inventories/prod/hosts_web
/// into  /home/user/repos/ansible/inventories/prod/hosts_*
fn parent_glob_pattern(path: &Path) -> String {
    path.parent()
        .map(|p| format!("{}/hosts_*", p.display()))
        .unwrap_or_else(|| path.display().to_string())
}

/// Matches any file that looks like an Ansible static inventory:
///   - named `hosts` or `hosts_*`
///   - named `inventory`, `production`, `staging`, `development`, `testing`
///   - ends with `.ini` and has an inventory-like name
fn is_ansible_inventory_file(path: &Path) -> bool {
    if !path.is_file() { return false; }
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };

    // Classic hosts file
    if name == "hosts" || name.starts_with("hosts_") {
        return true;
    }

    // Environment-named inventories (no extension)
    const ENV_NAMES: &[&str] = &[
        "inventory", "production", "staging", "development", "testing",
        "prod", "stage", "dev", "test", "local",
    ];
    if path.extension().is_none() && ENV_NAMES.contains(&name) {
        return looks_like_ansible_ini(path);
    }

    // .ini files with inventory-like names
    if name.ends_with(".ini") {
        let stem = name.trim_end_matches(".ini");
        if stem == "hosts" || stem == "inventory" || ENV_NAMES.contains(&stem) {
            return true;
        }
    }

    false
}

/// Quick heuristic: peek at a file to see if it looks like an INI-style
/// Ansible inventory (has [group] sections or bare hostnames/IPs).
fn looks_like_ansible_ini(path: &Path) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else { return false; };
    let mut lines = content.lines().filter(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with('#') && !t.starts_with(';')
    });
    // A non-empty file with at least one [group] header or a bare hostname
    lines.any(|l| {
        let t = l.trim();
        (t.starts_with('[') && t.ends_with(']'))
            || t.split_whitespace().next()
                .map(|tok| {
                    // bare hostname or IP (no = sign, no : in unusual places)
                    !tok.contains('=')
                        && (tok.chars().next().map(|c| c.is_alphanumeric()).unwrap_or(false))
                })
                .unwrap_or(false)
    })
}

fn is_yaml_inventory(path: &Path) -> bool {
    if !path.is_file() { return false; }
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    (name.ends_with(".yaml") || name.ends_with(".yml"))
        && (name.contains("host") || name.contains("inventory") || name.contains("sojourn"))
}

fn should_skip(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.') || SKIP_DIRS.contains(&n))
        .unwrap_or(false)
}

/// Extract a stable path key from a discovered source for dedup.
fn src_path_key(src: &DiscoveredSource) -> PathBuf {
    match &src.config {
        InventoryConfig::Ansible { path } => PathBuf::from(path),
        InventoryConfig::Yaml { path } => PathBuf::from(path),
        InventoryConfig::SshConfig { path } => PathBuf::from(path),
        InventoryConfig::ShellAlias { path } => PathBuf::from(path),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Search roots and patterns
// ─────────────────────────────────────────────────────────────────────────────

/// High-probability patterns checked before the deep scan
fn high_probability_paths() -> Vec<String> {
    let home = dirs::home_dir()
        .map(|h| h.display().to_string())
        .unwrap_or_default();

    vec![
        // ── ansible.cfg-adjacent inventory patterns ──────────────────────────
        // (ansible.cfg pass handled separately above; these catch unconfigured repos)

        // ── Classic hosts_* layout ───────────────────────────────────────────
        format!("{}/ansible/inventories/*/hosts_*", home),
        format!("{}/ansible/inventories/*/*/hosts_*", home),
        format!("{}/repos/ansible/inventories/*/hosts_*", home),
        format!("{}/repos/ansible/inventories/*/*/hosts_*", home),
        format!("{}/repos/*/inventories/*/hosts_*", home),
        format!("{}/repos/*/inventories/*/*/hosts_*", home),
        format!("{}/repos/*/inventory/hosts_*", home),
        format!("{}/repos/*/*/inventories/*/hosts_*", home),
        format!("{}/projects/*/inventories/*/hosts_*", home),
        format!("{}/work/*/inventories/*/hosts_*", home),

        // ── Bare hosts file at repo root / one level down ────────────────────
        format!("{}/repos/*/hosts", home),
        format!("{}/repos/*/*/hosts", home),
        format!("{}/repos/*/inventory/hosts", home),
        format!("{}/repos/*/inventories/hosts", home),
        format!("{}/ansible/hosts", home),

        // ── .ini inventories ─────────────────────────────────────────────────
        format!("{}/repos/*/hosts.ini", home),
        format!("{}/repos/*/inventory.ini", home),
        format!("{}/repos/*/inventory/hosts.ini", home),
        format!("{}/repos/*/inventories/hosts.ini", home),

        // ── Environment-named (no extension) ────────────────────────────────
        format!("{}/repos/*/inventory/production", home),
        format!("{}/repos/*/inventory/staging", home),
        format!("{}/repos/*/inventory/development", home),
        format!("{}/repos/*/inventories/production", home),
        format!("{}/repos/*/inventories/staging", home),
        format!("{}/repos/*/inventories/development", home),

        // ── YAML inventories ─────────────────────────────────────────────────
        format!("{}/repos/*/hosts.yaml", home),
        format!("{}/repos/*/hosts.yml", home),
        format!("{}/repos/*/inventory.yaml", home),
        format!("{}/repos/*/inventory.yml", home),
        format!("{}/repos/*/*/hosts.yaml", home),
        format!("{}/repos/*/*/hosts.yml", home),

        // ── Sojourn own config ───────────────────────────────────────────────
        format!("{}/.config/sojourn/hosts.yaml", home),
    ]
}

/// Roots to walk during the deep scan
fn repo_scan_roots() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    vec![
        home.join("repos"),
        home.join("projects"),
        home.join("work"),
        home.join("src"),
        home.join("dev"),
        home.join("ansible"),
        home.join("infrastructure"),
        home.join("infra"),
        home.join("ops"),
    ]
    .into_iter()
    .filter(|p| p.exists())
    .collect()
}

fn shorten(s: &str, max: usize) -> String {
    let home = dirs::home_dir()
        .map(|h| h.display().to_string())
        .unwrap_or_default();
    let s = s.replace(&home, "~");
    if s.len() <= max {
        s
    } else {
        format!("...{}", &s[s.len().saturating_sub(max - 3)..])
    }
}
