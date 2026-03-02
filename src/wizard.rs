/// First-run setup wizard.
///
/// Shown automatically when no config file exists, or when the user runs
/// `sojourn setup`. Runs a background inventory scan while displaying a live
/// progress screen, then lets the user review/toggle discovered sources,
/// optionally add IP labels, and finally writes ~/.config/sojourn/config.toml.
use std::io;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph},
    Frame, Terminal,
};

use crate::config::{Config, InventoryConfig, IpLabel, Settings};
use crate::discovery::{discover_async, DiscoveredSource, SCAN_TIMEOUT};

// ─────────────────────────────────────────────────────────────────────────────

const C_CYAN: Color = Color::Cyan;
const C_YELLOW: Color = Color::Yellow;
const C_GREEN: Color = Color::Green;
const C_MAGENTA: Color = Color::Magenta;
const C_GRAY: Color = Color::DarkGray;
const C_WHITE: Color = Color::White;
const C_RED: Color = Color::Red;
const C_DIM: Color = Color::Rgb(80, 80, 80);

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum WizardStep {
    /// Background scan running, showing spinner + live results
    Scanning,
    /// Scan done (or timed out), user reviews sources
    ReviewSources,
    /// User is typing a manual path to add
    ManualAdd,
    /// Optional: add IP range labels
    IpLabels,
    /// Typing a new IP label entry
    AddIpLabel,
    /// Done — config written, ready to launch
    Done,
}

#[allow(dead_code)]
struct IpLabelEntry {
    pattern: String,
    label: String,
    editing_field: IpLabelField,
    pattern_input: String,
    label_input: String,
}

#[derive(Clone)]
enum IpLabelField {
    Pattern,
    Label,
}

pub struct Wizard {
    step: WizardStep,
    sources: Vec<DiscoveredSource>,
    scan_rx: Option<mpsc::Receiver<DiscoveredSource>>,
    scan_start: Instant,
    scan_done: bool,
    cursor: usize,
    spinner_tick: usize,
    // manual add
    manual_input: String,
    manual_error: Option<String>,
    // ip labels
    ip_labels: Vec<IpLabel>,
    ip_label_cursor: usize,
    label_entry: Option<IpLabelEntry>,
}

impl Wizard {
    pub fn new() -> Self {
        Self {
            step: WizardStep::Scanning,
            sources: Vec::new(),
            scan_rx: Some(discover_async()),
            scan_start: Instant::now(),
            scan_done: false,
            cursor: 0,
            spinner_tick: 0,
            manual_input: String::new(),
            manual_error: None,
            ip_labels: default_ip_labels(),
            ip_label_cursor: 0,
            label_entry: None,
        }
    }

    /// Run the wizard to completion. Returns the resulting Config on success,
    /// or None if the user quit without saving.
    pub fn run(mut self) -> anyhow::Result<Option<Config>> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.event_loop(&mut terminal);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> anyhow::Result<Option<Config>> {
        loop {
            // Poll for new scan results (non-blocking)
            self.drain_scan_results();

            terminal.draw(|f| self.render(f))?;

            if self.step == WizardStep::Done {
                return Ok(Some(self.build_config()));
            }

            // Use a short poll timeout so the spinner animates smoothly
            if event::poll(Duration::from_millis(80))? {
                if let Event::Key(key) = event::read()? {
                    if let Some(result) = self.handle_key(key)? {
                        return Ok(result);
                    }
                }
            }

            // Advance spinner
            self.spinner_tick = self.spinner_tick.wrapping_add(1);

            // Auto-advance from Scanning to ReviewSources when scan finishes
            if self.step == WizardStep::Scanning
                && self.scan_done
            {
                self.step = WizardStep::ReviewSources;
                self.cursor = 0;
            }
        }
    }

    fn drain_scan_results(&mut self) {
        if self.scan_done {
            return;
        }
        if let Some(rx) = &self.scan_rx {
            loop {
                match rx.try_recv() {
                    Ok(src) => {
                        // Deduplicate by config path
                        let already = self.sources.iter().any(|s| {
                            config_path(&s.config) == config_path(&src.config)
                        });
                        if !already {
                            self.sources.push(src);
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.scan_done = true;
                        self.scan_rx = None;
                        break;
                    }
                }
            }
            // Force-finish after timeout even if thread hasn't disconnected yet
            if self.scan_start.elapsed() >= SCAN_TIMEOUT + Duration::from_millis(500) {
                self.scan_done = true;
                self.scan_rx = None;
            }
        }
    }

    fn handle_key(
        &mut self,
        key: crossterm::event::KeyEvent,
    ) -> anyhow::Result<Option<Option<Config>>> {
        use KeyCode::*;
        use KeyModifiers as KM;

        match (&self.step.clone(), key.code) {
            // ── Quit from anywhere ────────────────────────────────────────
            (_, Char('c')) if key.modifiers.contains(KM::CONTROL) => {
                return Ok(Some(None)); // user quit
            }

            // ── Scanning: skip wait ───────────────────────────────────────
            (WizardStep::Scanning, Enter) | (WizardStep::Scanning, Char(' ')) => {
                self.scan_done = true;
                self.step = WizardStep::ReviewSources;
            }

            // ── ReviewSources ─────────────────────────────────────────────
            (WizardStep::ReviewSources, Up) | (WizardStep::ReviewSources, Char('k')) => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            (WizardStep::ReviewSources, Down) | (WizardStep::ReviewSources, Char('j')) => {
                // +1 for the "Add manually" row
                if self.cursor < self.sources.len() {
                    self.cursor += 1;
                }
            }
            (WizardStep::ReviewSources, Char(' ')) => {
                if self.cursor < self.sources.len() {
                    self.sources[self.cursor].selected = !self.sources[self.cursor].selected;
                }
            }
            (WizardStep::ReviewSources, Enter) => {
                if self.cursor == self.sources.len() {
                    // "Add manually" row
                    self.step = WizardStep::ManualAdd;
                    self.manual_input.clear();
                    self.manual_error = None;
                } else {
                    // Move to next step
                    self.step = WizardStep::IpLabels;
                    self.ip_label_cursor = 0;
                }
            }
            (WizardStep::ReviewSources, Char('n')) => {
                // Skip to done without IP labels
                self.step = WizardStep::Done;
            }

            // ── ManualAdd ─────────────────────────────────────────────────
            (WizardStep::ManualAdd, Esc) => {
                self.step = WizardStep::ReviewSources;
            }
            (WizardStep::ManualAdd, Backspace) => {
                self.manual_input.pop();
            }
            (WizardStep::ManualAdd, Enter) => {
                self.commit_manual_add();
            }
            (WizardStep::ManualAdd, Char(c)) => {
                self.manual_input.push(c);
                self.manual_error = None;
            }

            // ── IpLabels ──────────────────────────────────────────────────
            (WizardStep::IpLabels, Up) | (WizardStep::IpLabels, Char('k')) => {
                if self.ip_label_cursor > 0 {
                    self.ip_label_cursor -= 1;
                }
            }
            (WizardStep::IpLabels, Down) | (WizardStep::IpLabels, Char('j')) => {
                if self.ip_label_cursor < self.ip_labels.len() {
                    self.ip_label_cursor += 1;
                }
            }
            (WizardStep::IpLabels, Char('d')) | (WizardStep::IpLabels, Delete) => {
                if self.ip_label_cursor < self.ip_labels.len() {
                    self.ip_labels.remove(self.ip_label_cursor);
                    if self.ip_label_cursor > 0 {
                        self.ip_label_cursor -= 1;
                    }
                }
            }
            (WizardStep::IpLabels, Enter) => {
                if self.ip_label_cursor == self.ip_labels.len() {
                    // "Add new" row — open inline editor
                    self.label_entry = Some(IpLabelEntry {
                        pattern: String::new(),
                        label: String::new(),
                        editing_field: IpLabelField::Pattern,
                        pattern_input: String::new(),
                        label_input: String::new(),
                    });
                    self.step = WizardStep::AddIpLabel;
                } else {
                    // Done button
                    self.step = WizardStep::Done;
                }
            }
            (WizardStep::IpLabels, Char('s')) | (WizardStep::IpLabels, Char('n')) => {
                self.step = WizardStep::Done;
            }

            // ── AddIpLabel ────────────────────────────────────────────────
            (WizardStep::AddIpLabel, Esc) => {
                self.label_entry = None;
                self.step = WizardStep::IpLabels;
            }
            (WizardStep::AddIpLabel, Tab) => {
                if let Some(entry) = &mut self.label_entry {
                    entry.editing_field = match entry.editing_field {
                        IpLabelField::Pattern => IpLabelField::Label,
                        IpLabelField::Label => IpLabelField::Pattern,
                    };
                }
            }
            (WizardStep::AddIpLabel, Backspace) => {
                if let Some(entry) = &mut self.label_entry {
                    match entry.editing_field {
                        IpLabelField::Pattern => { entry.pattern_input.pop(); }
                        IpLabelField::Label => { entry.label_input.pop(); }
                    }
                }
            }
            (WizardStep::AddIpLabel, Enter) => {
                self.commit_ip_label();
            }
            (WizardStep::AddIpLabel, Char(c)) => {
                if let Some(entry) = &mut self.label_entry {
                    match entry.editing_field {
                        IpLabelField::Pattern => entry.pattern_input.push(c),
                        IpLabelField::Label => entry.label_input.push(c),
                    }
                }
            }

            _ => {}
        }

        Ok(None)
    }

    fn commit_manual_add(&mut self) {
        let raw = self.manual_input.trim().to_string();
        if raw.is_empty() {
            self.manual_error = Some("Path cannot be empty.".into());
            return;
        }
        let expanded = shellexpand::tilde(&raw).to_string();

        // Determine type
        let (config, type_label) = if expanded.contains('*') {
            (InventoryConfig::Ansible { path: expanded.clone() }, "ansible")
        } else if expanded.ends_with(".yaml") || expanded.ends_with(".yml") {
            (InventoryConfig::Yaml { path: expanded.clone() }, "yaml")
        } else {
            (InventoryConfig::Ansible { path: format!("{}*/hosts_*", expanded.trim_end_matches('/').to_string() + "/") }, "ansible")
        };

        // Quick probe
        use crate::inventory::InventorySource;
        let count = match &config {
            InventoryConfig::Ansible { path } => {
                crate::inventory::ansible::AnsibleInventory { pattern: path.clone() }
                    .load().unwrap_or_default().len()
            }
            InventoryConfig::Yaml { path } => {
                crate::inventory::yaml_config::YamlInventory { path: path.clone() }
                    .load().unwrap_or_default().len()
            }
            _ => 0,
        };

        if count == 0 {
            self.manual_error = Some(format!(
                "No hosts found at '{}'. Check the path and try again.", raw
            ));
            return;
        }

        self.sources.push(DiscoveredSource {
            label: format!("{}  ({} hosts, {})", raw, count, type_label),
            config,
            host_count: count,
            selected: true,
        });
        self.step = WizardStep::ReviewSources;
        self.cursor = self.sources.len().saturating_sub(1);
    }

    fn commit_ip_label(&mut self) {
        if let Some(entry) = self.label_entry.take() {
            let pattern = entry.pattern_input.trim().to_string();
            let label = entry.label_input.trim().to_string();
            if !pattern.is_empty() && !label.is_empty() {
                self.ip_labels.push(IpLabel {
                    pattern,
                    label,
                    color: None,
                });
                self.ip_label_cursor = self.ip_labels.len(); // point at "Add" row
            }
        }
        self.step = WizardStep::IpLabels;
    }

    fn build_config(&self) -> Config {
        let inventory: Vec<InventoryConfig> = self
            .sources
            .iter()
            .filter(|s| s.selected)
            .map(|s| s.config.clone())
            .collect();

        Config {
            settings: Settings::default(),
            inventory,
            ip_labels: self.ip_labels.clone(),
            jump_hosts: vec![],
            host_overrides: vec![],
        }
    }

    // ── Rendering ─────────────────────────────────────────────────────────────

    fn render(&self, f: &mut Frame) {
        let area = f.area();

        // Centered popup
        let w = 72u16.min(area.width.saturating_sub(4));
        let h = 36u16.min(area.height.saturating_sub(2));
        let x = (area.width.saturating_sub(w)) / 2;
        let y = (area.height.saturating_sub(h)) / 2;
        let popup = Rect::new(x, y, w, h);

        f.render_widget(Clear, popup);

        match self.step {
            WizardStep::Scanning => self.render_scanning(f, popup),
            WizardStep::ReviewSources => self.render_review(f, popup),
            WizardStep::ManualAdd => {
                self.render_review(f, popup);
                self.render_manual_add_overlay(f, area);
            }
            WizardStep::IpLabels => self.render_ip_labels(f, popup),
            WizardStep::AddIpLabel => {
                self.render_ip_labels(f, popup);
                self.render_add_ip_label_overlay(f, area);
            }
            WizardStep::Done => {}
        }
    }

    fn render_scanning(&self, f: &mut Frame, area: Rect) {
        let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let spin = spinner_chars[self.spinner_tick % spinner_chars.len()];
        let elapsed = self.scan_start.elapsed().as_secs();
        let remaining = SCAN_TIMEOUT.as_secs().saturating_sub(elapsed);

        let block = Block::default()
            .title(Span::styled(
                " ⚡ sojourn — First Run Setup ",
                Style::default().fg(C_CYAN).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(C_CYAN));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),  // intro text
                Constraint::Length(2),  // spinner line
                Constraint::Min(0),     // live results
                Constraint::Length(2),  // hint
            ])
            .split(inner);

        // Intro
        let intro = Paragraph::new(Text::from(vec![
            Line::raw(""),
            Line::from(vec![
                Span::raw("  Welcome! Let's find your hosts. "),
                Span::styled(
                    "Scanning your system...",
                    Style::default().fg(C_YELLOW),
                ),
            ]),
        ]));
        f.render_widget(intro, layout[0]);

        // Spinner + timer
        let spin_line = Line::from(vec![
            Span::styled(
                format!("  {} ", spin),
                Style::default().fg(C_CYAN).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "Searching inventory files  ({} sources found, {}s left)",
                    self.sources.len(),
                    remaining
                ),
                Style::default().fg(C_GRAY),
            ),
        ]);
        f.render_widget(Paragraph::new(spin_line), layout[1]);

        // Live results as they arrive
        let items: Vec<ListItem> = self
            .sources
            .iter()
            .map(|s| {
                ListItem::new(Line::from(vec![
                    Span::styled("  ✓ ", Style::default().fg(C_GREEN)),
                    Span::styled(s.label.clone(), Style::default().fg(C_WHITE)),
                ]))
            })
            .collect();
        let list = List::new(items);
        f.render_widget(list, layout[2]);

        // Hint
        let hint = Paragraph::new(Line::from(vec![
            Span::styled(
                "  [Enter] skip wait   [Ctrl+C] quit",
                Style::default().fg(C_DIM),
            ),
        ]));
        f.render_widget(hint, layout[3]);
    }

    fn render_review(&self, f: &mut Frame, area: Rect) {
        let timeout_note = if self.scan_start.elapsed() >= SCAN_TIMEOUT {
            " (scan timed out)"
        } else {
            ""
        };

        let block = Block::default()
            .title(Span::styled(
                " ⚡ sojourn — Inventory Sources ",
                Style::default().fg(C_CYAN).add_modifier(Modifier::BOLD),
            ))
            .title_bottom(Span::styled(
                format!(" {} source(s) found{} ", self.sources.len(), timeout_note),
                Style::default().fg(C_GRAY),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(C_CYAN));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(inner);

        // Header text
        let header = Paragraph::new(Text::from(vec![
            Line::raw(""),
            Line::from(vec![
                Span::raw("  Select which inventory sources to load. "),
                Span::styled("[Space]", Style::default().fg(C_YELLOW)),
                Span::raw(" toggles, "),
                Span::styled("[Enter]", Style::default().fg(C_YELLOW)),
                Span::raw(" continues."),
            ]),
        ]));
        f.render_widget(header, layout[0]);

        // Source list + "Add manually" row
        let mut items: Vec<ListItem> = self
            .sources
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let is_sel = self.cursor == i;
                let check = if s.selected {
                    Span::styled("[✓] ", Style::default().fg(C_GREEN).add_modifier(Modifier::BOLD))
                } else {
                    Span::styled("[ ] ", Style::default().fg(C_GRAY))
                };
                let arrow = if is_sel {
                    Span::styled("▶ ", Style::default().fg(C_CYAN))
                } else {
                    Span::raw("  ")
                };
                let text = Span::styled(
                    s.label.clone(),
                    if is_sel {
                        Style::default().fg(C_WHITE).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(C_GRAY)
                    },
                );
                let style = if is_sel {
                    Style::default().bg(Color::Rgb(25, 50, 80))
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![arrow, check, text])).style(style)
            })
            .collect();

        // "Add manually" row
        let add_sel = self.cursor == self.sources.len();
        items.push(
            ListItem::new(Line::from(vec![
                Span::styled(
                    if add_sel { "▶ " } else { "  " },
                    Style::default().fg(C_CYAN),
                ),
                Span::styled(
                    "+ Add a path manually...",
                    if add_sel {
                        Style::default().fg(C_YELLOW).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(C_DIM)
                    },
                ),
            ]))
            .style(if add_sel {
                Style::default().bg(Color::Rgb(25, 50, 80))
            } else {
                Style::default()
            }),
        );

        f.render_widget(List::new(items), layout[1]);

        // Bottom hint
        let hint = Paragraph::new(Line::from(vec![
            Span::styled("  [↑↓/jk]", Style::default().fg(C_YELLOW)),
            Span::raw(" move  "),
            Span::styled("[Space]", Style::default().fg(C_YELLOW)),
            Span::raw(" toggle  "),
            Span::styled("[Enter]", Style::default().fg(C_YELLOW)),
            Span::raw(" continue  "),
            Span::styled("[n]", Style::default().fg(C_YELLOW)),
            Span::raw(" skip to save"),
        ]));
        f.render_widget(hint, layout[2]);
    }

    fn render_manual_add_overlay(&self, f: &mut Frame, area: Rect) {
        let w = 60u16.min(area.width.saturating_sub(4));
        let h = 8u16;
        let x = (area.width.saturating_sub(w)) / 2;
        let y = (area.height.saturating_sub(h)) / 2;
        let popup = Rect::new(x, y, w, h);
        f.render_widget(Clear, popup);

        let block = Block::default()
            .title(Span::styled(
                " Add Inventory Path ",
                Style::default().fg(C_YELLOW).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(C_YELLOW));

        let inner = block.inner(popup);
        f.render_widget(block, popup);

        let mut lines = vec![
            Line::raw(""),
            Line::from(vec![
                Span::raw("  Path: "),
                Span::styled(
                    format!("{}█", self.manual_input),
                    Style::default().fg(C_WHITE).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::raw(""),
            Line::from(Span::styled(
                "  Supports globs: ~/repos/ansible/inventories/*/hosts_*",
                Style::default().fg(C_DIM),
            )),
        ];

        if let Some(err) = &self.manual_error {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                format!("  ⚠ {}", err),
                Style::default().fg(C_RED),
            )));
        }

        f.render_widget(Paragraph::new(Text::from(lines)), inner);
    }

    fn render_ip_labels(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(Span::styled(
                " ⚡ sojourn — IP Range Labels  (optional) ",
                Style::default().fg(C_CYAN).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(C_CYAN));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(inner);

        let header = Paragraph::new(Text::from(vec![
            Line::raw(""),
            Line::from(vec![
                Span::raw("  Map IP ranges to location names — e.g. "),
                Span::styled("10.0.*", Style::default().fg(C_YELLOW)),
                Span::raw(" → "),
                Span::styled("Home Lab", Style::default().fg(C_MAGENTA)),
            ]),
            Line::from(Span::styled(
                "  Shown next to every host that matches.",
                Style::default().fg(C_GRAY),
            )),
        ]));
        f.render_widget(header, layout[0]);

        let mut items: Vec<ListItem> = self
            .ip_labels
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let is_sel = self.ip_label_cursor == i;
                let arrow = if is_sel { "▶ " } else { "  " };
                let style = if is_sel {
                    Style::default().bg(Color::Rgb(25, 50, 80))
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(arrow, Style::default().fg(C_CYAN)),
                    Span::styled(
                        format!("{:<20}", l.pattern),
                        Style::default().fg(C_YELLOW),
                    ),
                    Span::styled(" → ", Style::default().fg(C_GRAY)),
                    Span::styled(l.label.clone(), Style::default().fg(C_MAGENTA)),
                    Span::styled(
                        if is_sel { "  [d] delete" } else { "" },
                        Style::default().fg(C_DIM),
                    ),
                ]))
                .style(style)
            })
            .collect();

        // "Add new" row
        let add_sel = self.ip_label_cursor == self.ip_labels.len();
        items.push(
            ListItem::new(Line::from(vec![
                Span::styled(
                    if add_sel { "▶ " } else { "  " },
                    Style::default().fg(C_CYAN),
                ),
                Span::styled(
                    "+ Add IP range label...",
                    if add_sel {
                        Style::default().fg(C_YELLOW).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(C_DIM)
                    },
                ),
            ]))
            .style(if add_sel {
                Style::default().bg(Color::Rgb(25, 50, 80))
            } else {
                Style::default()
            }),
        );

        // Spacer + "Save & Launch" row at end
        let done_sel = self.ip_label_cursor > self.ip_labels.len();
        items.push(ListItem::new(Line::raw("")));
        items.push(
            ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "  Save config & launch sojourn  ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(if done_sel { C_GREEN } else { C_CYAN })
                        .add_modifier(Modifier::BOLD),
                ),
            ]))
        );

        f.render_widget(List::new(items), layout[1]);

        let hint = Paragraph::new(Line::from(vec![
            Span::styled("  [↑↓/jk]", Style::default().fg(C_YELLOW)),
            Span::raw(" move  "),
            Span::styled("[Enter]", Style::default().fg(C_YELLOW)),
            Span::raw(" add/select  "),
            Span::styled("[d]", Style::default().fg(C_YELLOW)),
            Span::raw(" delete  "),
            Span::styled("[s/n]", Style::default().fg(C_YELLOW)),
            Span::raw(" skip & save"),
        ]));
        f.render_widget(hint, layout[2]);
    }

    fn render_add_ip_label_overlay(&self, f: &mut Frame, area: Rect) {
        let w = 60u16.min(area.width.saturating_sub(4));
        let h = 10u16;
        let x = (area.width.saturating_sub(w)) / 2;
        let y = (area.height.saturating_sub(h)) / 2;
        let popup = Rect::new(x, y, w, h);
        f.render_widget(Clear, popup);

        let block = Block::default()
            .title(Span::styled(
                " Add IP Range Label ",
                Style::default().fg(C_YELLOW).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(C_YELLOW));

        let inner = block.inner(popup);
        f.render_widget(block, popup);

        if let Some(entry) = &self.label_entry {
            let pattern_active = matches!(entry.editing_field, IpLabelField::Pattern);
            let label_active = matches!(entry.editing_field, IpLabelField::Label);

            let lines = vec![
                Line::raw(""),
                Line::from(vec![
                    Span::raw("  IP Pattern : "),
                    Span::styled(
                        format!(
                            "{}{}",
                            entry.pattern_input,
                            if pattern_active { "█" } else { "" }
                        ),
                        Style::default()
                            .fg(if pattern_active { C_WHITE } else { C_GRAY })
                            .add_modifier(if pattern_active { Modifier::BOLD } else { Modifier::empty() }),
                    ),
                ]),
                Line::raw(""),
                Line::from(vec![
                    Span::raw("  Label      : "),
                    Span::styled(
                        format!(
                            "{}{}",
                            entry.label_input,
                            if label_active { "█" } else { "" }
                        ),
                        Style::default()
                            .fg(if label_active { C_MAGENTA } else { C_GRAY })
                            .add_modifier(if label_active { Modifier::BOLD } else { Modifier::empty() }),
                    ),
                ]),
                Line::raw(""),
                Line::from(Span::styled(
                    "  [Tab] switch field   [Enter] save   [Esc] cancel",
                    Style::default().fg(C_DIM),
                )),
                Line::raw(""),
                Line::from(Span::styled(
                    "  Example pattern: 10.80.*  or  10.80.0.0/16",
                    Style::default().fg(C_DIM),
                )),
            ];

            f.render_widget(Paragraph::new(Text::from(lines)), inner);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

fn config_path(c: &InventoryConfig) -> String {
    match c {
        InventoryConfig::Ansible { path } => path.clone(),
        InventoryConfig::SshConfig { path } => path.clone(),
        InventoryConfig::Yaml { path } => path.clone(),
        InventoryConfig::ShellAlias { path } => path.clone(),
    }
}

/// Sensible default IP labels shown pre-filled in the wizard.
/// The user can delete or add to these.
fn default_ip_labels() -> Vec<IpLabel> {
    vec![
        IpLabel { pattern: "192.168.*".into(), label: "Local Network".into(), color: Some("green".into()) },
        IpLabel { pattern: "10.0.*".into(),    label: "Private (10.0)".into(), color: None },
        IpLabel { pattern: "172.16.*".into(),  label: "Private (172.16)".into(), color: None },
    ]
}

/// Write config to the default path, creating parent dirs as needed.
pub fn write_config(config: &Config) -> anyhow::Result<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
    let config_dir = home.join(".config/sojourn");
    std::fs::create_dir_all(&config_dir)?;
    let config_path = config_dir.join("config.toml");
    let content = toml::to_string_pretty(config)?;
    std::fs::write(&config_path, content)?;
    // Restrict to owner read/write only (contains host inventory paths)
    std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600))?;
    Ok(config_path)
}
