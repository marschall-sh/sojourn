use ratatui::style::Color;

/// All color slots used by the UI. Every theme fills these.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Theme {
    pub name: &'static str,

    // ── Backgrounds ──────────────────────────────────────────────────────────
    pub bg:          Color,  // main background (used where terminals don't fill)
    pub bg_selected: Color,  // selected row highlight

    // ── Foregrounds ──────────────────────────────────────────────────────────
    pub fg:          Color,  // normal text (hostnames, values)
    pub fg_dim:      Color,  // dimmed text (source paths, hints, inactive)

    // ── Borders & chrome ─────────────────────────────────────────────────────
    pub border:         Color,  // inactive panel border
    pub border_active:  Color,  // focused panel border

    // ── Search bar ───────────────────────────────────────────────────────────
    pub search_border:  Color,  // search box border when active
    pub search_cursor:  Color,  // blinking cursor character
    pub match_char:     Color,  // fuzzy-matched characters highlighted in list

    // ── Host list ────────────────────────────────────────────────────────────
    pub cursor_marker:  Color,  // ▶ arrow on selected row
    pub multi_marker:   Color,  // ● dot on multi-selected rows
    pub ip:             Color,  // IP address text
    pub location:       Color,  // IP-range label e.g. "Home Lab", "Office VPN"
    pub custom_label:   Color,  // user-set ✎ label on a host

    // ── Details pane ─────────────────────────────────────────────────────────
    pub detail_label:   Color,  // section headings ("Hostname", "IP Address" …)
    pub detail_value:   Color,  // values under headings
    pub jump_host:      Color,  // jump host value
    pub source_path:    Color,  // inventory source path (dim)
    pub group_bg:       Color,  // group tag pill background
    pub group_fg:       Color,  // group tag pill foreground

    // ── Header band ──────────────────────────────────────────────────────────
    pub header_band:    Color,  // column-header row background (slightly darker than bg)

    // ── Accents & actions ────────────────────────────────────────────────────
    pub accent:         Color,  // title, primary accent (⚡ sojourn)
    pub key_hint:       Color,  // [key] hints in status bar
    pub success:        Color,  // ✓ messages
    pub warning:        Color,  // ⚠ messages
    pub error:          Color,  // error / destructive
}

impl Theme {
    /// Resolve a theme by name from config. Falls back to Default on unknown names.
    pub fn by_name(name: &str) -> &'static Theme {
        match name.to_lowercase().replace([' ', '_'], "-").as_str() {
            "tokyo-night" | "tokyonight"              => &TOKYO_NIGHT,
            "catppuccin"  | "catppuccin-mocha"        => &CATPPUCCIN_MOCHA,
            "catppuccin-latte" | "latte"              => &CATPPUCCIN_LATTE,
            "dracula"                                  => &DRACULA,
            "gruvbox"     | "gruvbox-dark"            => &GRUVBOX_DARK,
            "nord"                                     => &NORD,
            "solarized"   | "solarized-light"         => &SOLARIZED_LIGHT,
            "rose-pine"   | "rose-pine-dawn" | "rosepine" => &ROSE_PINE_DAWN,
            _                                          => &DEFAULT,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Default — pure terminal colors, works on every background
// ─────────────────────────────────────────────────────────────────────────────
pub static DEFAULT: Theme = Theme {
    name: "default",
    bg:             Color::Reset,
    bg_selected:    Color::Rgb(30, 60, 100),
    fg:             Color::White,
    fg_dim:         Color::DarkGray,
    border:         Color::DarkGray,
    border_active:  Color::Cyan,
    search_border:  Color::Yellow,
    search_cursor:  Color::Yellow,
    match_char:     Color::Yellow,
    cursor_marker:  Color::Cyan,
    multi_marker:   Color::Red,
    ip:             Color::Green,
    location:       Color::Magenta,
    custom_label:   Color::Yellow,
    detail_label:   Color::DarkGray,
    detail_value:   Color::White,
    jump_host:      Color::Yellow,
    source_path:    Color::DarkGray,
    group_bg:       Color::Blue,
    group_fg:       Color::White,
    header_band:    Color::Rgb(0x1e, 0x22, 0x2a),  // dark navy band
    accent:         Color::Cyan,
    key_hint:       Color::Yellow,
    success:        Color::Green,
    warning:        Color::Yellow,
    error:          Color::Red,
};

// ─────────────────────────────────────────────────────────────────────────────
// Tokyo Night (Storm variant)
// https://github.com/enkia/tokyo-night-vscode-theme
// ─────────────────────────────────────────────────────────────────────────────
pub static TOKYO_NIGHT: Theme = Theme {
    name: "tokyo-night",
    bg:             Color::Rgb(0x1d, 0x20, 0x2f),  // #1d202f  storm bg
    bg_selected:    Color::Rgb(0x2f, 0x33, 0x54),  // #2f3354  selection
    fg:             Color::Rgb(0xc0, 0xca, 0xf5),  // #c0caf5  foreground
    fg_dim:         Color::Rgb(0x56, 0x5f, 0x89),  // #565f89  comment
    border:         Color::Rgb(0x3b, 0x42, 0x61),  // #3b4261  non-text
    border_active:  Color::Rgb(0x7a, 0xa2, 0xf7),  // #7aa2f7  blue
    search_border:  Color::Rgb(0xe0, 0xaf, 0x68),  // #e0af68  yellow
    search_cursor:  Color::Rgb(0xe0, 0xaf, 0x68),  // #e0af68  yellow
    match_char:     Color::Rgb(0xff, 0x9e, 0x64),  // #ff9e64  orange
    cursor_marker:  Color::Rgb(0x7a, 0xa2, 0xf7),  // #7aa2f7  blue
    multi_marker:   Color::Rgb(0xf7, 0x76, 0x8e),  // #f7768e  red
    ip:             Color::Rgb(0x9e, 0xce, 0x6a),  // #9ece6a  green
    location:       Color::Rgb(0xbb, 0x9a, 0xf7),  // #bb9af7  purple
    custom_label:   Color::Rgb(0xe0, 0xaf, 0x68),  // #e0af68  yellow
    detail_label:   Color::Rgb(0x56, 0x5f, 0x89),  // #565f89  comment
    detail_value:   Color::Rgb(0xc0, 0xca, 0xf5),  // #c0caf5  fg
    jump_host:      Color::Rgb(0xe0, 0xaf, 0x68),  // #e0af68  yellow
    source_path:    Color::Rgb(0x56, 0x5f, 0x89),  // #565f89  comment
    group_bg:       Color::Rgb(0x7a, 0xa2, 0xf7),  // #7aa2f7  blue
    group_fg:       Color::Rgb(0x1d, 0x20, 0x2f),  // #1d202f  bg
    header_band:    Color::Rgb(0x1a, 0x1d, 0x2a),  // darker than storm bg
    accent:         Color::Rgb(0x7a, 0xa2, 0xf7),  // #7aa2f7  blue
    key_hint:       Color::Rgb(0xe0, 0xaf, 0x68),  // #e0af68  yellow
    success:        Color::Rgb(0x9e, 0xce, 0x6a),  // #9ece6a  green
    warning:        Color::Rgb(0xe0, 0xaf, 0x68),  // #e0af68  yellow
    error:          Color::Rgb(0xf7, 0x76, 0x8e),  // #f7768e  red
};

// ─────────────────────────────────────────────────────────────────────────────
// Catppuccin Mocha
// https://github.com/catppuccin/catppuccin
// ─────────────────────────────────────────────────────────────────────────────
pub static CATPPUCCIN_MOCHA: Theme = Theme {
    name: "catppuccin-mocha",
    bg:             Color::Rgb(0x1e, 0x1e, 0x2e),  // #1e1e2e  base
    bg_selected:    Color::Rgb(0x31, 0x32, 0x44),  // #313244  surface0
    fg:             Color::Rgb(0xcd, 0xd6, 0xf4),  // #cdd6f4  text
    fg_dim:         Color::Rgb(0x58, 0x5b, 0x70),  // #585b70  surface2
    border:         Color::Rgb(0x45, 0x47, 0x5a),  // #45475a  surface1
    border_active:  Color::Rgb(0x89, 0xb4, 0xfa),  // #89b4fa  blue
    search_border:  Color::Rgb(0xf9, 0xe2, 0xaf),  // #f9e2af  yellow
    search_cursor:  Color::Rgb(0xf9, 0xe2, 0xaf),  // #f9e2af  yellow
    match_char:     Color::Rgb(0xfa, 0xb3, 0x87),  // #fab387  peach
    cursor_marker:  Color::Rgb(0x89, 0xb4, 0xfa),  // #89b4fa  blue
    multi_marker:   Color::Rgb(0xf3, 0x8b, 0xa8),  // #f38ba8  red
    ip:             Color::Rgb(0xa6, 0xe3, 0xa1),  // #a6e3a1  green
    location:       Color::Rgb(0xcb, 0xa6, 0xf7),  // #cba6f7  mauve
    custom_label:   Color::Rgb(0xf9, 0xe2, 0xaf),  // #f9e2af  yellow
    detail_label:   Color::Rgb(0x58, 0x5b, 0x70),  // #585b70  surface2
    detail_value:   Color::Rgb(0xcd, 0xd6, 0xf4),  // #cdd6f4  text
    jump_host:      Color::Rgb(0xf9, 0xe2, 0xaf),  // #f9e2af  yellow
    source_path:    Color::Rgb(0x58, 0x5b, 0x70),  // #585b70  dim
    group_bg:       Color::Rgb(0x89, 0xb4, 0xfa),  // #89b4fa  blue
    group_fg:       Color::Rgb(0x1e, 0x1e, 0x2e),  // #1e1e2e  base
    header_band:    Color::Rgb(0x18, 0x18, 0x25),  // #181825  mantle
    accent:         Color::Rgb(0x89, 0xb4, 0xfa),  // #89b4fa  blue
    key_hint:       Color::Rgb(0xf9, 0xe2, 0xaf),  // #f9e2af  yellow
    success:        Color::Rgb(0xa6, 0xe3, 0xa1),  // #a6e3a1  green
    warning:        Color::Rgb(0xf9, 0xe2, 0xaf),  // #f9e2af  yellow
    error:          Color::Rgb(0xf3, 0x8b, 0xa8),  // #f38ba8  red
};

// ─────────────────────────────────────────────────────────────────────────────
// Dracula
// https://draculatheme.com
// ─────────────────────────────────────────────────────────────────────────────
pub static DRACULA: Theme = Theme {
    name: "dracula",
    bg:             Color::Rgb(0x28, 0x2a, 0x36),  // #282a36  background
    bg_selected:    Color::Rgb(0x44, 0x47, 0x5a),  // #44475a  selection
    fg:             Color::Rgb(0xf8, 0xf8, 0xf2),  // #f8f8f2  foreground
    fg_dim:         Color::Rgb(0x62, 0x72, 0xa4),  // #6272a4  comment
    border:         Color::Rgb(0x44, 0x47, 0x5a),  // #44475a  selection
    border_active:  Color::Rgb(0xbd, 0x93, 0xf9),  // #bd93f9  purple
    search_border:  Color::Rgb(0xf1, 0xfa, 0x8c),  // #f1fa8c  yellow
    search_cursor:  Color::Rgb(0xf1, 0xfa, 0x8c),  // #f1fa8c  yellow
    match_char:     Color::Rgb(0xff, 0xb8, 0x6c),  // #ffb86c  orange
    cursor_marker:  Color::Rgb(0xbd, 0x93, 0xf9),  // #bd93f9  purple
    multi_marker:   Color::Rgb(0xff, 0x55, 0x55),  // #ff5555  red
    ip:             Color::Rgb(0x50, 0xfa, 0x7b),  // #50fa7b  green
    location:       Color::Rgb(0xff, 0x79, 0xc6),  // #ff79c6  pink
    custom_label:   Color::Rgb(0xf1, 0xfa, 0x8c),  // #f1fa8c  yellow
    detail_label:   Color::Rgb(0x62, 0x72, 0xa4),  // #6272a4  comment
    detail_value:   Color::Rgb(0xf8, 0xf8, 0xf2),  // #f8f8f2  fg
    jump_host:      Color::Rgb(0xf1, 0xfa, 0x8c),  // #f1fa8c  yellow
    source_path:    Color::Rgb(0x62, 0x72, 0xa4),  // #6272a4  comment
    group_bg:       Color::Rgb(0xbd, 0x93, 0xf9),  // #bd93f9  purple
    group_fg:       Color::Rgb(0x28, 0x2a, 0x36),  // #282a36  bg
    header_band:    Color::Rgb(0x21, 0x22, 0x2c),  // #21222c  darker bg
    accent:         Color::Rgb(0xbd, 0x93, 0xf9),  // #bd93f9  purple
    key_hint:       Color::Rgb(0xf1, 0xfa, 0x8c),  // #f1fa8c  yellow
    success:        Color::Rgb(0x50, 0xfa, 0x7b),  // #50fa7b  green
    warning:        Color::Rgb(0xf1, 0xfa, 0x8c),  // #f1fa8c  yellow
    error:          Color::Rgb(0xff, 0x55, 0x55),  // #ff5555  red
};

// ─────────────────────────────────────────────────────────────────────────────
// Gruvbox Dark (Material variant)
// https://github.com/morhetz/gruvbox
// ─────────────────────────────────────────────────────────────────────────────
pub static GRUVBOX_DARK: Theme = Theme {
    name: "gruvbox-dark",
    bg:             Color::Rgb(0x28, 0x28, 0x28),  // #282828  bg
    bg_selected:    Color::Rgb(0x3c, 0x38, 0x36),  // #3c3836  bg1
    fg:             Color::Rgb(0xeb, 0xdb, 0xb2),  // #ebdbb2  fg
    fg_dim:         Color::Rgb(0x66, 0x5c, 0x54),  // #665c54  bg4
    border:         Color::Rgb(0x50, 0x49, 0x45),  // #504945  bg2
    border_active:  Color::Rgb(0x83, 0xa5, 0x98),  // #83a598  aqua
    search_border:  Color::Rgb(0xfa, 0xbd, 0x2f),  // #fabd2f  yellow
    search_cursor:  Color::Rgb(0xfa, 0xbd, 0x2f),  // #fabd2f  yellow
    match_char:     Color::Rgb(0xfe, 0x80, 0x19),  // #fe8019  orange
    cursor_marker:  Color::Rgb(0x83, 0xa5, 0x98),  // #83a598  aqua
    multi_marker:   Color::Rgb(0xfb, 0x49, 0x34),  // #fb4934  red
    ip:             Color::Rgb(0xb8, 0xbb, 0x26),  // #b8bb26  green
    location:       Color::Rgb(0xd3, 0x86, 0x9b),  // #d3869b  purple
    custom_label:   Color::Rgb(0xfa, 0xbd, 0x2f),  // #fabd2f  yellow
    detail_label:   Color::Rgb(0x66, 0x5c, 0x54),  // #665c54  dim
    detail_value:   Color::Rgb(0xeb, 0xdb, 0xb2),  // #ebdbb2  fg
    jump_host:      Color::Rgb(0xfa, 0xbd, 0x2f),  // #fabd2f  yellow
    source_path:    Color::Rgb(0x66, 0x5c, 0x54),  // #665c54  dim
    group_bg:       Color::Rgb(0x83, 0xa5, 0x98),  // #83a598  aqua
    group_fg:       Color::Rgb(0x28, 0x28, 0x28),  // #282828  bg
    header_band:    Color::Rgb(0x1d, 0x20, 0x21),  // #1d2021  hard bg
    accent:         Color::Rgb(0x83, 0xa5, 0x98),  // #83a598  aqua
    key_hint:       Color::Rgb(0xfa, 0xbd, 0x2f),  // #fabd2f  yellow
    success:        Color::Rgb(0xb8, 0xbb, 0x26),  // #b8bb26  green
    warning:        Color::Rgb(0xfa, 0xbd, 0x2f),  // #fabd2f  yellow
    error:          Color::Rgb(0xfb, 0x49, 0x34),  // #fb4934  red
};

// ─────────────────────────────────────────────────────────────────────────────
// Nord
// https://www.nordtheme.com
// ─────────────────────────────────────────────────────────────────────────────
pub static NORD: Theme = Theme {
    name: "nord",
    bg:             Color::Rgb(0x2e, 0x34, 0x40),  // #2e3440  polar night 1
    bg_selected:    Color::Rgb(0x3b, 0x42, 0x52),  // #3b4252  polar night 2
    fg:             Color::Rgb(0xec, 0xef, 0xf4),  // #eceff4  snow storm 3
    fg_dim:         Color::Rgb(0x4c, 0x56, 0x6a),  // #4c566a  polar night 4
    border:         Color::Rgb(0x43, 0x4c, 0x5e),  // #434c5e  polar night 3
    border_active:  Color::Rgb(0x88, 0xc0, 0xd0),  // #88c0d0  frost 3
    search_border:  Color::Rgb(0xeb, 0xcb, 0x8b),  // #ebcb8b  aurora yellow
    search_cursor:  Color::Rgb(0xeb, 0xcb, 0x8b),  // #ebcb8b  aurora yellow
    match_char:     Color::Rgb(0xd0, 0x87, 0x70),  // #d08770  aurora orange
    cursor_marker:  Color::Rgb(0x88, 0xc0, 0xd0),  // #88c0d0  frost 3
    multi_marker:   Color::Rgb(0xbf, 0x61, 0x6a),  // #bf616a  aurora red
    ip:             Color::Rgb(0xa3, 0xbe, 0x8c),  // #a3be8c  aurora green
    location:       Color::Rgb(0xb4, 0x8e, 0xad),  // #b48ead  aurora purple
    custom_label:   Color::Rgb(0xeb, 0xcb, 0x8b),  // #ebcb8b  aurora yellow
    detail_label:   Color::Rgb(0x4c, 0x56, 0x6a),  // #4c566a  dim
    detail_value:   Color::Rgb(0xec, 0xef, 0xf4),  // #eceff4  fg
    jump_host:      Color::Rgb(0xeb, 0xcb, 0x8b),  // #ebcb8b  aurora yellow
    source_path:    Color::Rgb(0x4c, 0x56, 0x6a),  // #4c566a  dim
    group_bg:       Color::Rgb(0x5e, 0x81, 0xac),  // #5e81ac  frost 4
    group_fg:       Color::Rgb(0xec, 0xef, 0xf4),  // #eceff4  snow
    header_band:    Color::Rgb(0x25, 0x29, 0x33),  // darker polar night
    accent:         Color::Rgb(0x88, 0xc0, 0xd0),  // #88c0d0  frost 3
    key_hint:       Color::Rgb(0xeb, 0xcb, 0x8b),  // #ebcb8b  aurora yellow
    success:        Color::Rgb(0xa3, 0xbe, 0x8c),  // #a3be8c  green
    warning:        Color::Rgb(0xeb, 0xcb, 0x8b),  // #ebcb8b  yellow
    error:          Color::Rgb(0xbf, 0x61, 0x6a),  // #bf616a  red
};

// ─────────────────────────────────────────────────────────────────────────────
// Catppuccin Latte  (light sibling of Catppuccin Mocha)
// https://github.com/catppuccin/catppuccin
// ─────────────────────────────────────────────────────────────────────────────
pub static CATPPUCCIN_LATTE: Theme = Theme {
    name: "catppuccin-latte",
    bg:             Color::Rgb(0xef, 0xf1, 0xf5),  // #eff1f5  base
    bg_selected:    Color::Rgb(0xcc, 0xd0, 0xda),  // #ccd0da  surface1
    fg:             Color::Rgb(0x4c, 0x4f, 0x69),  // #4c4f69  text
    fg_dim:         Color::Rgb(0x8c, 0x8f, 0xa1),  // #8c8fa1  overlay1
    border:         Color::Rgb(0xbc, 0xc0, 0xcc),  // #bcc0cc  surface2
    border_active:  Color::Rgb(0x1e, 0x66, 0xf5),  // #1e66f5  blue
    search_border:  Color::Rgb(0xdf, 0x8e, 0x1d),  // #df8e1d  yellow
    search_cursor:  Color::Rgb(0xdf, 0x8e, 0x1d),  // #df8e1d  yellow
    match_char:     Color::Rgb(0xfe, 0x64, 0x0b),  // #fe640b  peach
    cursor_marker:  Color::Rgb(0x1e, 0x66, 0xf5),  // #1e66f5  blue
    multi_marker:   Color::Rgb(0xd2, 0x0f, 0x39),  // #d20f39  red
    ip:             Color::Rgb(0x40, 0xa0, 0x2b),  // #40a02b  green
    location:       Color::Rgb(0x88, 0x39, 0xef),  // #8839ef  mauve
    custom_label:   Color::Rgb(0xdf, 0x8e, 0x1d),  // #df8e1d  yellow
    detail_label:   Color::Rgb(0x8c, 0x8f, 0xa1),  // #8c8fa1  overlay1
    detail_value:   Color::Rgb(0x4c, 0x4f, 0x69),  // #4c4f69  text
    jump_host:      Color::Rgb(0xdf, 0x8e, 0x1d),  // #df8e1d  yellow
    source_path:    Color::Rgb(0x8c, 0x8f, 0xa1),  // #8c8fa1  overlay1
    group_bg:       Color::Rgb(0x1e, 0x66, 0xf5),  // #1e66f5  blue
    group_fg:       Color::Rgb(0xef, 0xf1, 0xf5),  // #eff1f5  base
    header_band:    Color::Rgb(0xe6, 0xe9, 0xef),  // #e6e9ef  mantle
    accent:         Color::Rgb(0x1e, 0x66, 0xf5),  // #1e66f5  blue
    key_hint:       Color::Rgb(0xdf, 0x8e, 0x1d),  // #df8e1d  yellow
    success:        Color::Rgb(0x40, 0xa0, 0x2b),  // #40a02b  green
    warning:        Color::Rgb(0xdf, 0x8e, 0x1d),  // #df8e1d  yellow
    error:          Color::Rgb(0xd2, 0x0f, 0x39),  // #d20f39  red
};

// ─────────────────────────────────────────────────────────────────────────────
// Solarized Light
// https://ethanschoonover.com/solarized/
// ─────────────────────────────────────────────────────────────────────────────
pub static SOLARIZED_LIGHT: Theme = Theme {
    name: "solarized-light",
    bg:             Color::Rgb(0xfd, 0xf6, 0xe3),  // #fdf6e3  base3
    bg_selected:    Color::Rgb(0xee, 0xe8, 0xd5),  // #eee8d5  base2
    fg:             Color::Rgb(0x65, 0x7b, 0x83),  // #657b83  base00
    fg_dim:         Color::Rgb(0x93, 0xa1, 0xa1),  // #93a1a1  base1
    border:         Color::Rgb(0x93, 0xa1, 0xa1),  // #93a1a1  base1
    border_active:  Color::Rgb(0x26, 0x8b, 0xd2),  // #268bd2  blue
    search_border:  Color::Rgb(0xb5, 0x89, 0x00),  // #b58900  yellow
    search_cursor:  Color::Rgb(0xb5, 0x89, 0x00),  // #b58900  yellow
    match_char:     Color::Rgb(0xcb, 0x4b, 0x16),  // #cb4b16  orange
    cursor_marker:  Color::Rgb(0x26, 0x8b, 0xd2),  // #268bd2  blue
    multi_marker:   Color::Rgb(0xdc, 0x32, 0x2f),  // #dc322f  red
    ip:             Color::Rgb(0x85, 0x99, 0x00),  // #859900  green
    location:       Color::Rgb(0xd3, 0x36, 0x82),  // #d33682  magenta
    custom_label:   Color::Rgb(0xb5, 0x89, 0x00),  // #b58900  yellow
    detail_label:   Color::Rgb(0x93, 0xa1, 0xa1),  // #93a1a1  base1
    detail_value:   Color::Rgb(0x65, 0x7b, 0x83),  // #657b83  base00
    jump_host:      Color::Rgb(0xb5, 0x89, 0x00),  // #b58900  yellow
    source_path:    Color::Rgb(0x93, 0xa1, 0xa1),  // #93a1a1  base1
    group_bg:       Color::Rgb(0x26, 0x8b, 0xd2),  // #268bd2  blue
    group_fg:       Color::Rgb(0xfd, 0xf6, 0xe3),  // #fdf6e3  base3
    header_band:    Color::Rgb(0xee, 0xe8, 0xd5),  // #eee8d5  base2
    accent:         Color::Rgb(0x26, 0x8b, 0xd2),  // #268bd2  blue
    key_hint:       Color::Rgb(0xb5, 0x89, 0x00),  // #b58900  yellow
    success:        Color::Rgb(0x85, 0x99, 0x00),  // #859900  green
    warning:        Color::Rgb(0xb5, 0x89, 0x00),  // #b58900  yellow
    error:          Color::Rgb(0xdc, 0x32, 0x2f),  // #dc322f  red
};

// ─────────────────────────────────────────────────────────────────────────────
// Rosé Pine Dawn  (light variant)
// https://rosepinetheme.com
// ─────────────────────────────────────────────────────────────────────────────
pub static ROSE_PINE_DAWN: Theme = Theme {
    name: "rose-pine-dawn",
    bg:             Color::Rgb(0xfa, 0xf4, 0xed),  // #faf4ed  base
    bg_selected:    Color::Rgb(0xf2, 0xe9, 0xe1),  // #f2e9e1  overlay
    fg:             Color::Rgb(0x57, 0x52, 0x79),  // #575279  text
    fg_dim:         Color::Rgb(0x98, 0x93, 0xa5),  // #9893a5  muted
    border:         Color::Rgb(0xdf, 0xda, 0xd9),  // #dfdad9  highlight med
    border_active:  Color::Rgb(0x28, 0x69, 0x83),  // #286983  pine
    search_border:  Color::Rgb(0xea, 0x9d, 0x34),  // #ea9d34  gold
    search_cursor:  Color::Rgb(0xea, 0x9d, 0x34),  // #ea9d34  gold
    match_char:     Color::Rgb(0xd7, 0x82, 0x7e),  // #d7827e  rose
    cursor_marker:  Color::Rgb(0x28, 0x69, 0x83),  // #286983  pine
    multi_marker:   Color::Rgb(0xb4, 0x63, 0x7a),  // #b4637a  love
    ip:             Color::Rgb(0x56, 0x94, 0x9f),  // #56949f  foam
    location:       Color::Rgb(0x90, 0x7a, 0xa9),  // #907aa9  iris
    custom_label:   Color::Rgb(0xea, 0x9d, 0x34),  // #ea9d34  gold
    detail_label:   Color::Rgb(0x98, 0x93, 0xa5),  // #9893a5  muted
    detail_value:   Color::Rgb(0x57, 0x52, 0x79),  // #575279  text
    jump_host:      Color::Rgb(0xea, 0x9d, 0x34),  // #ea9d34  gold
    source_path:    Color::Rgb(0x98, 0x93, 0xa5),  // #9893a5  muted
    group_bg:       Color::Rgb(0x28, 0x69, 0x83),  // #286983  pine
    group_fg:       Color::Rgb(0xfa, 0xf4, 0xed),  // #faf4ed  base
    header_band:    Color::Rgb(0xf2, 0xe9, 0xde),  // slightly darker than base
    accent:         Color::Rgb(0x28, 0x69, 0x83),  // #286983  pine
    key_hint:       Color::Rgb(0xea, 0x9d, 0x34),  // #ea9d34  gold
    success:        Color::Rgb(0x56, 0x94, 0x9f),  // #56949f  foam
    warning:        Color::Rgb(0xea, 0x9d, 0x34),  // #ea9d34  gold
    error:          Color::Rgb(0xb4, 0x63, 0x7a),  // #b4637a  love
};
