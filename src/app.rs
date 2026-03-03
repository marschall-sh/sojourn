use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::Backend, Terminal};
use std::collections::HashSet;
use std::process::Command;

use crate::config::{Config, HostOverride, InventoryConfig};
use crate::theme::Theme;
use dirs;
use crate::inventory::{
    ansible::AnsibleInventory,
    shell_alias::ShellAliasInventory,
    ssh_config::SshConfigInventory,
    yaml_config::YamlInventory,
    Host, InventorySource,
};
use crate::search::SearchEngine;
use crate::ui;

#[derive(Debug, Clone, PartialEq)]
pub enum EditField {
    User,
    Alias,
    Label,
    JumpHost,
}

impl EditField {
    pub fn next(&self) -> Self {
        match self {
            EditField::User     => EditField::Alias,
            EditField::Alias    => EditField::Label,
            EditField::Label    => EditField::JumpHost,
            EditField::JumpHost => EditField::User,
        }
    }
    pub fn prev(&self) -> Self {
        match self {
            EditField::User     => EditField::JumpHost,
            EditField::Alias    => EditField::User,
            EditField::Label    => EditField::Alias,
            EditField::JumpHost => EditField::Label,
        }
    }
}

/// State for the host-edit overlay
#[derive(Debug, Clone)]
pub struct EditState {
    pub host_idx: usize,       // index into App::hosts
    pub user_input: String,
    pub alias_input: String,
    pub label_input: String,
    pub jump_input: String,
    pub active_field: EditField,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Search,
    Navigate,
    Help,
    EditHost,
}

pub struct App {
    pub config: Config,
    pub theme: &'static Theme,
    pub hosts: Vec<Host>,
    /// (index into self.hosts, fuzzy score)
    pub filtered: Vec<(usize, i64)>,
    pub query: String,
    pub list_cursor: usize,
    pub list_scroll: usize,
    /// Visible list height — updated each frame by ui.rs, used for PgUp/PgDn
    pub list_page_size: usize,
    pub multi_selected: HashSet<usize>,
    pub mode: Mode,
    pub edit_state: Option<EditState>,
    pub status: Option<String>,
    pub should_quit: bool,
    search_engine: SearchEngine,
}

impl App {
    pub fn new(config: Config, initial_query: String) -> Result<Self> {
        let theme = Theme::by_name(&config.settings.theme);
        let mut app = Self {
            theme,
            config,
            hosts: Vec::new(),
            filtered: Vec::new(),
            query: initial_query,
            list_cursor: 0,
            list_scroll: 0,
            list_page_size: 20,
            multi_selected: HashSet::new(),
            mode: Mode::Search,
            edit_state: None,
            status: None,
            should_quit: false,
            search_engine: SearchEngine::new(),
        };

        app.load_inventory()?;
        app.update_filter();

        Ok(app)
    }

    fn load_inventory(&mut self) -> Result<()> {
        let inventory_configs = self.config.inventory.clone();

        for inv_config in &inventory_configs {
            match inv_config {
                InventoryConfig::Ansible { path } => {
                    let inv = AnsibleInventory {
                        pattern: path.clone(),
                    };
                    match inv.load() {
                        Ok(hosts) => self.hosts.extend(hosts),
                        Err(e) => {
                            self.status =
                                Some(format!("Warning: Ansible inventory failed: {}", e))
                        }
                    }
                }
                InventoryConfig::SshConfig { path } => {
                    let inv = SshConfigInventory { path: path.clone() };
                    match inv.load() {
                        Ok(hosts) => self.hosts.extend(hosts),
                        Err(e) => {
                            self.status = Some(format!("Warning: SSH config failed: {}", e))
                        }
                    }
                }
                InventoryConfig::Yaml { path } => {
                    let inv = YamlInventory { path: path.clone() };
                    match inv.load() {
                        Ok(hosts) => self.hosts.extend(hosts),
                        Err(e) => {
                            self.status = Some(format!("Warning: YAML config failed: {}", e))
                        }
                    }
                }
                InventoryConfig::ShellAlias { path } => {
                    let inv = ShellAliasInventory { path: path.clone() };
                    match inv.load() {
                        Ok(hosts) => self.hosts.extend(hosts),
                        Err(e) => {
                            self.status =
                                Some(format!("Warning: shell alias scan failed: {}", e))
                        }
                    }
                }
            }
        }

        // Apply jump host rules and default user from global config
        let rules = self.config.jump_hosts.clone();
        let default_user = self.config.settings.default_user.clone();

        for host in &mut self.hosts {
            if host.jump_host.is_none() {
                for rule in &rules {
                    if glob_match(&rule.host_pattern, &host.hostname) {
                        host.jump_host = Some(rule.jump_host.clone());
                        if host.user.is_none() {
                            host.user = rule.user.clone();
                        }
                        break;
                    }
                }
            }

            if host.user.is_none() {
                host.user = default_user.clone();
            }
        }

        // Apply per-host overrides saved via the TUI editor
        let overrides = self.config.host_overrides.clone();
        for host in &mut self.hosts {
            if let Some(ov) = overrides.iter().find(|o| o.hostname == host.hostname) {
                if let Some(u) = &ov.user      { host.user      = Some(u.clone()); }
                if let Some(a) = &ov.alias     { host.alias     = Some(a.clone()); }
                if let Some(l) = &ov.label     { host.label     = Some(l.clone()); }
                if let Some(j) = &ov.jump_host { host.jump_host = Some(j.clone()); }
            }
        }

        // Remove noise and duplicates
        self.hosts = clean_hosts(std::mem::take(&mut self.hosts));

        Ok(())
    }

    pub fn update_filter(&mut self) {
        if self.query.trim().is_empty() {
            self.filtered = (0..self.hosts.len()).map(|i| (i, 0i64)).collect();
            self.filtered
                .sort_by(|a, b| self.hosts[a.0].hostname.cmp(&self.hosts[b.0].hostname));
        } else {
            self.filtered = self.search_engine.search(&self.query, &self.hosts);
        }

        if self.list_cursor >= self.filtered.len() {
            self.list_cursor = self.filtered.len().saturating_sub(1);
        }
    }

    pub fn selected_host(&self) -> Option<&Host> {
        self.filtered
            .get(self.list_cursor)
            .map(|(idx, _)| &self.hosts[*idx])
    }

    pub fn move_up(&mut self) {
        if self.list_cursor > 0 {
            self.list_cursor -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.list_cursor + 1 < self.filtered.len() {
            self.list_cursor += 1;
        }
    }

    pub fn move_page_up(&mut self) {
        self.list_cursor = self.list_cursor.saturating_sub(self.list_page_size);
    }

    pub fn move_page_down(&mut self) {
        let max = self.filtered.len().saturating_sub(1);
        self.list_cursor = (self.list_cursor + self.list_page_size).min(max);
    }

    pub fn toggle_multi_select(&mut self) {
        if let Some((host_idx, _)) = self.filtered.get(self.list_cursor) {
            let idx = *host_idx;
            if self.multi_selected.contains(&idx) {
                self.multi_selected.remove(&idx);
            } else {
                self.multi_selected.insert(idx);
            }
        }
        self.move_down();
    }

    pub fn highlight_indices(&self, text: &str) -> Vec<usize> {
        if self.query.trim().is_empty() {
            return vec![];
        }
        self.search_engine.highlight_indices(text, &self.query)
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            terminal.draw(|f| ui::render(f, self))?;

            if self.should_quit {
                break;
            }

            if event::poll(std::time::Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key, terminal)?;
                }
            }
        }
        Ok(())
    }

    fn handle_key<B: Backend>(
        &mut self,
        key: crossterm::event::KeyEvent,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        use KeyCode::*;
        use KeyModifiers as KM;

        match self.mode {
            Mode::Help => match key.code {
                Esc | Char('q') | Char('?') | F(1) => self.mode = Mode::Navigate,
                _ => {}
            },

            Mode::Search => match (key.modifiers, key.code) {
                (_, Esc) => {
                    if self.query.is_empty() {
                        self.mode = Mode::Navigate;
                    } else {
                        self.query.clear();
                        self.update_filter();
                    }
                }
                (_, Backspace) => {
                    self.query.pop();
                    self.update_filter();
                }
                (KM::CONTROL, Char('u')) => {
                    self.query.clear();
                    self.update_filter();
                }
                (KM::CONTROL, Char('c')) | (KM::CONTROL, Char('d')) => {
                    self.should_quit = true;
                }
                (_, Down) | (KM::CONTROL, Char('n')) | (_, Tab) => {
                    self.mode = Mode::Navigate;
                    self.move_down();
                }
                (_, Up) | (KM::CONTROL, Char('p')) => {
                    self.mode = Mode::Navigate;
                    self.move_up();
                }
                (_, PageDown) => {
                    self.mode = Mode::Navigate;
                    self.move_page_down();
                }
                (_, PageUp) => {
                    self.mode = Mode::Navigate;
                    self.move_page_up();
                }
                (_, Enter) => {
                    if !self.filtered.is_empty() {
                        self.execute_ssh_and_return(terminal)?;
                    }
                }
                (_, Char(c)) => {
                    self.query.push(c);
                    self.update_filter();
                }
                _ => {}
            },

            Mode::Navigate => match (key.modifiers, key.code) {
                (_, Char('q')) | (KM::CONTROL, Char('c')) => {
                    self.should_quit = true;
                }
                (_, Char('?')) | (_, F(1)) => {
                    self.mode = Mode::Help;
                }
                (_, Char('/')) | (_, Char('i')) | (_, Char('s')) => {
                    self.mode = Mode::Search;
                }
                (_, Down) | (_, Char('j')) => {
                    self.move_down();
                }
                (_, Up) | (_, Char('k')) => {
                    self.move_up();
                }
                (_, PageDown) => {
                    self.move_page_down();
                }
                (_, PageUp) => {
                    self.move_page_up();
                }
                (_, Enter) => {
                    self.execute_ssh_and_return(terminal)?;
                }
                (_, Char(' ')) => {
                    self.toggle_multi_select();
                }
                (KM::CONTROL, Char('a')) => {
                    for (idx, _) in &self.filtered {
                        self.multi_selected.insert(*idx);
                    }
                }
                (KM::CONTROL, Char('d')) => {
                    self.multi_selected.clear();
                }
                // Open config in $EDITOR
                (_, Char('c')) => {
                    self.open_config_in_editor(terminal)?;
                }
                // Open host edit overlay
                (_, Char('e')) => {
                    self.open_edit_overlay();
                }
                _ => {}
            },

            Mode::EditHost => {
                if let Some(state) = self.edit_state.as_mut() {
                    match (key.modifiers, key.code) {
                        (_, Esc) => {
                            self.edit_state = None;
                            self.mode = Mode::Navigate;
                        }
                        (_, Tab) => {
                            state.active_field = state.active_field.next();
                        }
                        (_, BackTab) => {
                            state.active_field = state.active_field.prev();
                        }
                        (_, Backspace) => {
                            match state.active_field {
                                EditField::User     => { state.user_input.pop(); }
                                EditField::Alias    => { state.alias_input.pop(); }
                                EditField::Label    => { state.label_input.pop(); }
                                EditField::JumpHost => { state.jump_input.pop(); }
                            }
                        }
                        (KM::CONTROL, Char('u')) => {
                            match state.active_field {
                                EditField::User     => state.user_input.clear(),
                                EditField::Alias    => state.alias_input.clear(),
                                EditField::Label    => state.label_input.clear(),
                                EditField::JumpHost => state.jump_input.clear(),
                            }
                        }
                        (_, Enter) => {
                            self.save_edit_overlay()?;
                        }
                        (_, Char(c)) => {
                            match state.active_field {
                                EditField::User     => state.user_input.push(c),
                                EditField::Alias    => state.alias_input.push(c),
                                EditField::Label    => state.label_input.push(c),
                                EditField::JumpHost => state.jump_input.push(c),
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    fn open_edit_overlay(&mut self) {
        if let Some((host_idx, _)) = self.filtered.get(self.list_cursor) {
            let host = &self.hosts[*host_idx];
            self.edit_state = Some(EditState {
                host_idx: *host_idx,
                user_input:  host.user.clone().unwrap_or_default(),
                alias_input: host.alias.clone().unwrap_or_default(),
                label_input: host.label.clone().unwrap_or_default(),
                jump_input:  host.jump_host.clone().unwrap_or_default(),
                active_field: EditField::User,
            });
            self.mode = Mode::EditHost;
        }
    }

    fn save_edit_overlay(&mut self) -> Result<()> {
        let state = match self.edit_state.take() {
            Some(s) => s,
            None    => return Ok(()),
        };

        // Apply in-memory immediately
        let host = &mut self.hosts[state.host_idx];
        host.user      = if state.user_input.is_empty()  { None } else { Some(state.user_input.trim().to_string()) };
        host.alias     = if state.alias_input.is_empty() { None } else { Some(state.alias_input.trim().to_string()) };
        host.label     = if state.label_input.is_empty() { None } else { Some(state.label_input.trim().to_string()) };
        host.jump_host = if state.jump_input.is_empty()  { None } else { Some(state.jump_input.trim().to_string()) };

        // Persist to config
        let ov = HostOverride {
            hostname:  host.hostname.clone(),
            user:      host.user.clone(),
            alias:     host.alias.clone(),
            label:     host.label.clone(),
            jump_host: host.jump_host.clone(),
        };
        self.config.upsert_override(ov);
        crate::wizard::write_config(&self.config)?;

        self.status = Some(format!("Saved overrides for {}", self.hosts[state.host_idx].hostname));
        self.mode = Mode::Navigate;
        Ok(())
    }

    fn execute_ssh_and_return<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        if let Some(host) = self.selected_host().cloned() {
            // Leave alternate screen for SSH
            disable_raw_mode()?;
            execute!(
                std::io::stdout(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            terminal.show_cursor()?;

            println!("\r\n\x1b[1;36m⚡ Connecting to {} ...\x1b[0m\r\n", host.hostname);

            let exit_status = self.ssh_connect(&host);

            // If SSH exited immediately (e.g. host key check failed / changed),
            // pause so the user can read the error before the TUI redraws over it.
            let quick_exit = exit_status.as_ref()
                .map(|s| !s.success())
                .unwrap_or(true);

            if self.config.settings.exit_after_connect {
                // Exit sojourn entirely — just print a newline and return
                if quick_exit {
                    println!("\r\n\x1b[2m[Press any key to exit...]\x1b[0m");
                    let _ = std::io::stdin().read_line(&mut String::new());
                }
                self.should_quit = true;
            } else {
                if quick_exit {
                    println!("\r\n\x1b[2m[Press any key to return to sojourn...]\x1b[0m");
                    let _ = std::io::stdin().read_line(&mut String::new());
                } else {
                    println!("\r\n\x1b[2m[Disconnected. Returning to sojourn...]\x1b[0m\r\n");
                }
                // Re-enter TUI
                enable_raw_mode()?;
                execute!(
                    std::io::stdout(),
                    EnterAlternateScreen,
                    EnableMouseCapture
                )?;
                terminal.clear()?;
            }
        }
        Ok(())
    }

    fn ssh_connect(&self, host: &Host) -> Result<std::process::ExitStatus> {
        let mut args: Vec<String> = Vec::new();

        if let Some(jump) = &host.jump_host {
            args.push("-J".to_string());
            args.push(jump.clone());
        }

        if let Some(id_file) = &host.identity_file {
            args.push("-i".to_string());
            args.push(shellexpand::tilde(id_file).to_string());
        }

        if let Some(port) = host.port {
            args.push("-p".to_string());
            args.push(port.to_string());
        }

        if let Some(extra) = &self.config.settings.ssh_extra_args {
            for arg in extra.split_whitespace() {
                args.push(arg.to_string());
            }
        }

        args.push(host.connect_target());

        let status = Command::new("ssh").args(&args).status()?;
        Ok(status)
    }

    fn open_config_in_editor<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let home = dirs::home_dir().unwrap_or_default();
        let config_path = home.join(".config/sojourn/config.toml");

        if !config_path.exists() {
            self.status = Some("No config file found. Run `sojourn setup` first.".into());
            return Ok(());
        }

        let editor = std::env::var("EDITOR")
            .or_else(|_| std::env::var("VISUAL"))
            .unwrap_or_else(|_| "vi".into());

        disable_raw_mode()?;
        execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
        terminal.show_cursor()?;

        Command::new(&editor)
            .arg(config_path)
            .status()
            .ok();

        // Reload inventory after editing
        self.hosts.clear();
        self.load_inventory().ok();
        self.update_filter();
        self.status = Some("Config reloaded.".into());

        enable_raw_mode()?;
        execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        terminal.clear()?;

        Ok(())
    }
}

/// Remove noise entries and deduplicate hosts after loading all inventories.
fn clean_hosts(hosts: Vec<crate::inventory::Host>) -> Vec<crate::inventory::Host> {
    use std::collections::HashSet;

    // Hostnames/IPs that are never useful
    const NOISE: &[&str] = &[
        "127.0.0.1", "::1", "localhost", "localhost.localdomain",
        "0.0.0.0", "255.255.255.255",
    ];

    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(hosts.len());

    for host in hosts {
        // Drop loopback / wildcard / obviously useless entries
        let hostname_lc = host.hostname.to_lowercase();
        if NOISE.contains(&hostname_lc.as_str()) { continue; }
        if let Some(ip) = &host.ip {
            if NOISE.contains(&ip.as_str()) { continue; }
        }

        // Drop wildcard SSH config patterns (Host * or Host *.example.com)
        if host.hostname.contains('*') || host.hostname.contains('?') { continue; }

        // Drop entries that are just a bare hostname with no IP and no useful info —
        // keep them only if they have at least a port, user, jump, or look like an FQDN/IP
        let has_useful_addr = host.ip.is_some()
            || host.port.is_some()
            || host.jump_host.is_some()
            || host.user.is_some()
            || host.hostname.contains('.')
            || host.hostname.parse::<std::net::IpAddr>().is_ok();
        if !has_useful_addr { continue; }

        // Dedup key: prefer ip over hostname to catch same host under different names
        let dedup_key = format!(
            "{}|{}",
            host.ip.as_deref().unwrap_or(&host.hostname),
            host.user.as_deref().unwrap_or("")
        );
        if !seen.insert(dedup_key) { continue; }

        out.push(host);
    }

    out
}

fn glob_match(pattern: &str, s: &str) -> bool {
    let regex_str = regex::escape(pattern)
        .replace(r"\*", ".*")
        .replace(r"\?", ".");
    regex::Regex::new(&format!("^{}$", regex_str))
        .map(|r| r.is_match(s))
        .unwrap_or(false)
}
