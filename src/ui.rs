use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, EditField, Mode};
use crate::theme::Theme;

// ─────────────────────────────────────────────────────────────────────────────
pub fn render(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Outer layout: search bar | main | status bar
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // search bar (with rounded border)
            Constraint::Min(0),    // list + details
            Constraint::Length(1), // status bar
        ])
        .split(size);

    render_search_bar(f, app, outer[0]);

    // Main horizontal split: host list | details
    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(outer[1]);

    render_host_list(f, app, main[0]);
    render_host_details(f, app, main[1]);
    render_status_bar(f, app, outer[2]);

    if app.mode == Mode::Help {
        render_help_overlay(f, size, app.theme);
    }
    if app.mode == Mode::EditHost {
        render_edit_overlay(f, size, app);
    }
}

// ── Search bar ────────────────────────────────────────────────────────────────
// B2 style: rounded border (dim when idle, accent when searching),
// "sojourn  │  / query█" layout, count right-aligned.
fn render_search_bar(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;
    let active = app.mode == Mode::Search;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if active { t.border_active } else { t.border }));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let total = app.hosts.len();
    let shown = app.filtered.len();
    let count_str = format!("{}/{}", shown, total);
    let cursor    = if active { "█" } else { "" };

    // Compute padding so count is right-aligned
    let inner_w   = inner.width as usize;
    // Fixed sections: " sojourn  │  / " = 16 chars, "  {count}  " = count+4
    let fixed_len = 16 + app.query.chars().count() + cursor.chars().count() + count_str.chars().count() + 4;
    let pad       = inner_w.saturating_sub(fixed_len);

    let line = Line::from(vec![
        Span::raw(" "),
        Span::styled("sojourn", Style::default().fg(t.accent).add_modifier(Modifier::BOLD)),
        Span::styled("  │  ", Style::default().fg(t.border)),
        Span::styled("/ ", Style::default().fg(t.fg_dim)),
        Span::styled(app.query.clone(), Style::default().fg(t.fg).add_modifier(Modifier::BOLD)),
        Span::styled(cursor.to_string(), Style::default().fg(t.search_cursor)),
        Span::raw(" ".repeat(pad)),
        Span::styled(count_str, Style::default().fg(t.fg_dim)),
        Span::raw("  "),
    ]);

    f.render_widget(Paragraph::new(line), inner);
}

// ── Host list ─────────────────────────────────────────────────────────────────
// B2 style: no outer box, 1-row header band with column labels,
// then per-row Paragraph widgets. Selected row gets bg_selected + ▎ accent bar.
fn render_host_list(f: &mut Frame, app: &mut App, area: Rect) {
    let t = app.theme;

    if area.height < 2 {
        return;
    }

    // Split: 1-row header + content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    let header_area = chunks[0];
    let list_area   = chunks[1];

    // Column proportions (relative to full area width)
    let w            = area.width as usize;
    let marker_w     = 2usize;                          // "▎ " or "  "
    let name_field_w = ((w * 44) / 100).max(8);         // hostname column
    let addr_field_w = ((w * 22) / 100).max(8);         // address column
    // location column gets the remainder

    // ── Header band ──────────────────────────────────────────────────────────
    let header = Line::from(vec![
        Span::raw(" ".repeat(marker_w)),
        Span::styled(
            format!("{:<width$}", "NAME", width = name_field_w),
            Style::default().fg(t.fg_dim).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:<width$}", "ADDRESS", width = addr_field_w),
            Style::default().fg(t.fg_dim).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "LOCATION",
            Style::default().fg(t.fg_dim).add_modifier(Modifier::BOLD),
        ),
    ]);
    f.render_widget(
        Paragraph::new(header).style(Style::default().bg(t.header_band)),
        header_area,
    );

    // ── Empty state ───────────────────────────────────────────────────────────
    if app.filtered.is_empty() {
        let msg = if app.hosts.is_empty() {
            "No hosts loaded.\nEdit ~/.config/sojourn/config.toml\nto add inventory sources."
        } else {
            "No matches found.\nTry a different search query."
        };
        f.render_widget(
            Paragraph::new(msg)
                .style(Style::default().fg(t.fg_dim))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            list_area,
        );
        return;
    }

    // ── Scroll adjustment ─────────────────────────────────────────────────────
    let list_height = list_area.height as usize;
    let cursor      = app.list_cursor;

    if cursor < app.list_scroll {
        app.list_scroll = cursor;
    } else if cursor >= app.list_scroll + list_height {
        app.list_scroll = cursor - list_height + 1;
    }

    let scroll = app.list_scroll;

    // Pre-collect row indices to satisfy the borrow checker
    let rows: Vec<(usize, usize)> = app
        .filtered
        .iter()
        .enumerate()
        .skip(scroll)
        .take(list_height)
        .map(|(vis_idx, (host_idx, _))| (vis_idx, *host_idx))
        .collect();

    // ── Render each row ───────────────────────────────────────────────────────
    for (vis_idx, host_idx) in rows {
        let host      = &app.hosts[host_idx];
        let is_cursor = vis_idx == cursor;
        let is_multi  = app.multi_selected.contains(&host_idx);

        let row_y    = list_area.y + (vis_idx - scroll) as u16;
        let row_rect = Rect::new(list_area.x, row_y, list_area.width, 1);

        // Left accent: ▎ on cursor, ● on multi-selected, spaces otherwise
        let marker = if is_cursor {
            Span::styled("▎ ", Style::default().fg(t.cursor_marker))
        } else if is_multi {
            Span::styled("● ", Style::default().fg(t.multi_marker))
        } else {
            Span::raw("  ")
        };

        // Hostname — truncate to column width and apply fuzzy highlights
        let display_name = truncate_str(&host.hostname, name_field_w.saturating_sub(1));
        let name_chars   = display_name.chars().count();
        let name_pad     = name_field_w.saturating_sub(name_chars);
        let name_spans   = highlight_text(&display_name, &app.query, app, t.fg, t.match_char);

        // ADDRESS column — alias takes priority over IP
        let ip_text    = host.alias.as_deref()
            .or(host.ip.as_deref())
            .unwrap_or("—");
        let display_ip = truncate_str(ip_text, addr_field_w.saturating_sub(1));

        // Location label
        let location = host
            .ip
            .as_deref()
            .and_then(|ip| app.config.label_for_ip(ip))
            .map(|l| l.label.as_str())
            .unwrap_or("");

        // Assemble spans
        let mut spans: Vec<Span> = vec![marker];
        spans.extend(name_spans);
        if name_pad > 0 {
            spans.push(Span::raw(" ".repeat(name_pad)));
        }
        spans.push(Span::styled(
            format!("{:<width$}", display_ip, width = addr_field_w),
            Style::default().fg(t.ip),
        ));
        spans.push(Span::styled(
            location.to_string(),
            Style::default().fg(t.location),
        ));

        // Optional custom label (appended dimly after location)
        if let Some(lbl) = &host.label {
            spans.push(Span::styled(
                format!(" ✎ {}", lbl),
                Style::default()
                    .fg(t.custom_label)
                    .add_modifier(Modifier::ITALIC),
            ));
        }

        let row_style = if is_cursor {
            Style::default().bg(t.bg_selected)
        } else {
            Style::default()
        };

        f.render_widget(
            Paragraph::new(Line::from(spans)).style(row_style),
            row_rect,
        );
    }
}

// ── Host details ──────────────────────────────────────────────────────────────
// B2 style: Borders::LEFT only (acts as panel divider), header band,
// then detail content below.
fn render_host_details(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;

    // Only left border as vertical divider
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(t.border));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    // Split inner: 1-row header band + content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);

    // Header band — matches the list header band
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "DETAILS",
                Style::default().fg(t.fg_dim).add_modifier(Modifier::BOLD),
            ),
        ]))
        .style(Style::default().bg(t.header_band)),
        chunks[0],
    );

    let content_area = chunks[1];

    let Some(host) = app.selected_host() else {
        f.render_widget(
            Paragraph::new("Select a host to view details")
                .style(Style::default().fg(t.fg_dim))
                .alignment(Alignment::Center),
            content_area,
        );
        return;
    };

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::raw(""));

    // Custom label (prominent at top if set)
    if let Some(lbl) = &host.label {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("✎  {}", lbl),
                Style::default()
                    .fg(t.custom_label)
                    .add_modifier(Modifier::BOLD | Modifier::ITALIC),
            ),
        ]));
        lines.push(Line::raw(""));
    }

    // Hostname
    lines.push(label_line("Hostname", t));
    lines.push(value_line(&host.hostname, t.fg, true));
    lines.push(Line::raw(""));

    // IP Address + location
    lines.push(label_line("IP Address", t));
    let ip_str = host.ip.as_deref().unwrap_or("(none)");
    lines.push(value_line(ip_str, t.ip, false));
    if let Some(ip) = &host.ip {
        if let Some(label) = app.config.label_for_ip(ip) {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    label.label.clone(),
                    Style::default().fg(t.location).add_modifier(Modifier::BOLD),
                ),
            ]));
        }
    }
    lines.push(Line::raw(""));

    // User
    if let Some(user) = &host.user {
        lines.push(label_line("User", t));
        lines.push(value_line(user, t.accent, false));
        lines.push(Line::raw(""));
    }

    // Port
    if let Some(port) = host.port {
        lines.push(label_line("Port", t));
        lines.push(value_line(&port.to_string(), t.key_hint, false));
        lines.push(Line::raw(""));
    }

    // Jump host
    if let Some(jump) = &host.jump_host {
        lines.push(label_line("Jump Host", t));
        lines.push(value_line(jump, t.jump_host, false));
        lines.push(Line::raw(""));
    }

    // Identity file
    if let Some(id) = &host.identity_file {
        lines.push(label_line("Identity File", t));
        lines.push(value_line(id, t.source_path, false));
        lines.push(Line::raw(""));
    }

    // Groups
    if !host.groups.is_empty() {
        lines.push(label_line("Groups", t));
        let group_spans: Vec<Span> = host
            .groups
            .iter()
            .flat_map(|g| {
                vec![
                    Span::styled(
                        format!(" {} ", g),
                        Style::default()
                            .fg(t.group_fg)
                            .bg(t.group_bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                ]
            })
            .collect();
        lines.push(Line::from(
            std::iter::once(Span::raw("  "))
                .chain(group_spans)
                .collect::<Vec<_>>(),
        ));
        lines.push(Line::raw(""));
    }

    // Tags
    if !host.tags.is_empty() {
        lines.push(label_line("Tags", t));
        for (k, v) in &host.tags {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("{}: ", k), Style::default().fg(t.fg_dim)),
                Span::styled(v.clone(), Style::default().fg(t.fg)),
            ]));
        }
        lines.push(Line::raw(""));
    }

    // Source
    lines.push(label_line("Source", t));
    let source_short = shorten_path(&host.source, content_area.width as usize - 4);
    lines.push(value_line(&source_short, t.source_path, false));
    lines.push(Line::raw(""));

    // Connect command
    lines.push(label_line("Connect", t));
    let cmd = format!("ssh {}", host.connect_target());
    lines.push(value_line(&cmd, t.accent, false));
    lines.push(Line::raw(""));

    // Action hints at bottom
    lines.push(Line::from(Span::styled(
        "─".repeat(content_area.width.saturating_sub(2) as usize),
        Style::default().fg(t.fg_dim),
    )));
    lines.push(Line::from(vec![
        Span::styled(
            "  [Enter] ",
            Style::default().fg(t.key_hint).add_modifier(Modifier::BOLD),
        ),
        Span::styled("Connect  ", Style::default().fg(t.fg)),
        Span::styled("[e] ", Style::default().fg(t.key_hint).add_modifier(Modifier::BOLD)),
        Span::styled("Edit  ", Style::default().fg(t.fg)),
        Span::styled(
            "[Space] ",
            Style::default().fg(t.key_hint).add_modifier(Modifier::BOLD),
        ),
        Span::styled("Select", Style::default().fg(t.fg)),
    ]));

    f.render_widget(
        Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
        content_area,
    );
}

// ── Helper: detail section label ──────────────────────────────────────────────
fn label_line(label: &str, t: &Theme) -> Line<'static> {
    Line::from(vec![Span::styled(
        format!("  {}", label),
        Style::default()
            .fg(t.detail_label)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )])
}

fn value_line(value: &str, color: Color, bold: bool) -> Line<'static> {
    let mut style = Style::default().fg(color);
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    Line::from(vec![Span::raw("  "), Span::styled(value.to_string(), style)])
}

fn shorten_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        return path.to_string();
    }
    let shortened = shellexpand::tilde(path).to_string();
    if shortened.len() <= max_len {
        return shortened;
    }
    let start = shortened.len().saturating_sub(max_len - 3);
    format!("...{}", &shortened[start..])
}

/// Truncate a string to `max_chars` Unicode scalar values,
/// appending `…` if truncation occurred.
fn truncate_str(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else if max_chars > 1 {
        let truncated: String = chars[..max_chars - 1].iter().collect();
        format!("{}…", truncated)
    } else {
        chars[..1].iter().collect()
    }
}

/// Produce spans with fuzzy-match characters highlighted.
fn highlight_text(
    text: &str,
    _query: &str,
    app: &App,
    normal_color: Color,
    highlight_color: Color,
) -> Vec<Span<'static>> {
    let indices = app.highlight_indices(text);
    if indices.is_empty() {
        return vec![Span::styled(
            text.to_string(),
            Style::default().fg(normal_color),
        )];
    }

    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();
    let mut current_normal = String::new();
    let idx_set: std::collections::HashSet<usize> = indices.into_iter().collect();

    for (i, ch) in chars.iter().enumerate() {
        if idx_set.contains(&i) {
            if !current_normal.is_empty() {
                spans.push(Span::styled(
                    current_normal.clone(),
                    Style::default().fg(normal_color),
                ));
                current_normal.clear();
            }
            spans.push(Span::styled(
                ch.to_string(),
                Style::default()
                    .fg(highlight_color)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            current_normal.push(*ch);
        }
    }

    if !current_normal.is_empty() {
        spans.push(Span::styled(
            current_normal,
            Style::default().fg(normal_color),
        ));
    }

    spans
}

// ── Status bar ────────────────────────────────────────────────────────────────
// B2 style: dot-separated hints right-aligned, status/multi count left-aligned.
fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;
    let multi_count = app.multi_selected.len();

    let (left_text, left_style) = if let Some(status) = &app.status {
        (
            format!(" {}", status),
            Style::default().fg(t.success),
        )
    } else if multi_count > 0 {
        (
            format!(" {} selected", multi_count),
            Style::default().fg(t.multi_marker).add_modifier(Modifier::BOLD),
        )
    } else {
        (String::new(), Style::default())
    };

    let hints = match app.mode {
        Mode::Search   => " ↑↓ move  ·  Enter connect  ·  Esc clear  ·  Tab list ",
        Mode::Navigate => " ↑↓/jk move  ·  Enter connect  ·  e edit  ·  / search  ·  ? help  ·  q quit ",
        Mode::EditHost => " Tab next  ·  Enter save  ·  Esc cancel ",
        Mode::Help     => " ? / Esc  close ",
    };

    // Right-align hints across the full status bar
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(hints, Style::default().fg(t.fg_dim))))
            .alignment(Alignment::Right),
        area,
    );

    // Overlay left-aligned status on the left third (won't trample hints on wide terminals)
    if !left_text.is_empty() {
        let left_w   = (left_text.chars().count() as u16 + 1).min(area.width / 2);
        let left_rect = Rect::new(area.x, area.y, left_w, area.height);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(left_text, left_style))),
            left_rect,
        );
    }
}

// ── Help overlay ──────────────────────────────────────────────────────────────
fn render_help_overlay(f: &mut Frame, area: Rect, t: &Theme) {
    let popup_width  = 60u16.min(area.width.saturating_sub(4));
    let popup_height = 24u16.min(area.height.saturating_sub(4));
    let popup_x      = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y      = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area   = Rect::new(popup_x, popup_y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(Span::styled(
            " ⚡ sojourn — Keyboard Shortcuts ",
            Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(t.accent));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let lines = vec![
        help_section("SEARCH", t),
        help_row("/  or  i", "Focus the search box", t),
        help_row("Type anything", "Filter hosts with fuzzy match", t),
        help_row("Backspace", "Delete last character", t),
        help_row("Ctrl+U", "Clear search query", t),
        help_row("Esc", "Clear search / back to list", t),
        Line::raw(""),
        help_section("NAVIGATION", t),
        help_row("↑ / k", "Move cursor up", t),
        help_row("↓ / j", "Move cursor down", t),
        help_row("Tab", "Switch focus search ↔ list", t),
        Line::raw(""),
        help_section("CONNECTIONS", t),
        help_row("Enter", "SSH connect to selected host", t),
        help_row("e", "Edit host — set user, label, jump host", t),
        help_row("Space", "Toggle multi-select on host", t),
        help_row("Ctrl+A", "Select all visible hosts", t),
        help_row("Ctrl+D", "Clear all selections", t),
        Line::raw(""),
        help_section("GENERAL", t),
        help_row("?  or  F1", "Toggle this help screen", t),
        help_row("q  or  Ctrl+C", "Quit sojourn", t),
        Line::raw(""),
        Line::from(Span::styled(
            "  Inventory sources: Ansible · SSH Config · YAML",
            Style::default().fg(t.fg_dim).add_modifier(Modifier::ITALIC),
        )),
    ];

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn help_section(title: &str, t: &Theme) -> Line<'static> {
    Line::from(vec![Span::styled(
        format!("  {} ", title),
        Style::default()
            .fg(t.accent)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )])
}

fn help_row(key: &str, desc: &str, t: &Theme) -> Line<'static> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("{:<18}", key),
            Style::default().fg(t.key_hint).add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc.to_string(), Style::default().fg(t.fg)),
    ])
}

// ── Host edit overlay ─────────────────────────────────────────────────────────
fn render_edit_overlay(f: &mut Frame, area: Rect, app: &App) {
    let t = app.theme;
    let state = match &app.edit_state {
        Some(s) => s,
        None    => return,
    };
    let host = &app.hosts[state.host_idx];

    let w  = 58u16.min(area.width.saturating_sub(4));
    let h  = 16u16;
    let x  = (area.width.saturating_sub(w)) / 2;
    let y  = (area.height.saturating_sub(h)) / 2;
    let popup = Rect::new(x, y, w, h);

    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(
            format!(" ✎ Edit: {} ", host.hostname),
            Style::default().fg(t.key_hint).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.key_hint));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let active_style   = Style::default().fg(t.fg).add_modifier(Modifier::BOLD);
    let inactive_style = Style::default().fg(t.fg_dim);
    let cursor         = "█";

    let field_line = |label: &str, value: &str, active: bool| -> Line<'static> {
        let label_span = Span::styled(
            format!("  {:<12}", label),
            if active {
                Style::default().fg(t.key_hint).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg_dim)
            },
        );
        let value_span = Span::styled(
            if active { format!("{}{}", value, cursor) } else { value.to_string() },
            if active { active_style } else { inactive_style },
        );
        Line::from(vec![label_span, value_span])
    };

    let lines = vec![
        Line::raw(""),
        field_line("User",      &state.user_input,  state.active_field == EditField::User),
        Line::raw(""),
        field_line("Alias",     &state.alias_input, state.active_field == EditField::Alias),
        Line::raw(""),
        field_line("Label",     &state.label_input, state.active_field == EditField::Label),
        Line::raw(""),
        field_line("Jump Host", &state.jump_input,  state.active_field == EditField::JumpHost),
        Line::raw(""),
        Line::from(Span::styled(
            "  Leave blank to clear a field",
            Style::default().fg(t.fg_dim).add_modifier(Modifier::ITALIC),
        )),
        Line::raw(""),
        Line::from(vec![
            Span::styled("  [Tab] ",   Style::default().fg(t.key_hint)),
            Span::raw("next field  "),
            Span::styled("[Enter] ",   Style::default().fg(t.key_hint)),
            Span::raw("save  "),
            Span::styled("[Esc] ",     Style::default().fg(t.key_hint)),
            Span::raw("cancel"),
        ]),
    ];

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}
