# sojourn

> *sojourn* /ˈsoʊdʒərn/ — a temporary stay in a place. Because every shell session is just passing through.

A fast, minimal TUI for managing SSH hosts — built in Rust. Fuzzy-search across thousands of hosts, hit Enter, you're in.

![sojourn demo](assets/demo.gif)

---

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/marschall-sh/sojourn/main/install.sh | bash
```

Installs to `~/.local/bin`. Supports **macOS (Apple Silicon)**, **Linux (x86-64)**, and **Linux (arm64)**.

Or build from source:

```bash
cargo install --git https://github.com/marschall-sh/sojourn
```

---

## Getting started

On first run, sojourn launches a setup wizard that auto-discovers SSH configs, Ansible inventories, and shell aliases on your machine:

```bash
sojourn setup
```

Select your inventory sources, configure IP range labels, and optionally enable integrations like Teleport. Re-running `sojourn setup` at any time pre-fills all existing settings — you only change what you need.

---

## Features

- **Fuzzy search** across all hosts as you type — hostname, IP, group, tag
- **Multiple inventory sources** — SSH config, Ansible inventories, shell aliases, custom YAML
- **IP range labels** — map `10.0.*` → `Home Lab`, `10.10.*` → `Office VPN`
- **Jump host support** — auto-wires `ProxyJump` based on host patterns
- **Multi-select** — open connections to several hosts at once
- **9 built-in themes** — six dark, three light
- **Plugins** — extend sojourn with integrations like [Teleport (tsh)](docs/teleport.md)

---

## Plugins

| Plugin | Description | Docs |
|--------|-------------|------|
| **Teleport** | Browse and connect to Teleport-managed hosts alongside SSH hosts. Login picker, session management, source filter. | [→ docs/teleport.md](docs/teleport.md) |

Enable plugins during `sojourn setup` or by editing `~/.config/sojourn/config.toml` directly.

---

## Keybindings

| Key | Action |
|-----|--------|
| `/` or start typing | Search |
| `↑↓` / `j` `k` | Navigate |
| `Enter` | Connect |
| `Space` | Multi-select |
| `f` | Cycle filter: all → ssh → teleport |
| `e` | Edit host |
| `Ctrl+A` / `Ctrl+D` | Select all / clear |
| `?` | Help |
| `q` | Quit |

---

## Config

The wizard writes `~/.config/sojourn/config.toml` for you. You can also edit it directly:

```toml
[settings]
default_user = "ubuntu"
theme = "tokyo-night"
connect_on_single_match = true

[[inventory]]
type = "ssh_config"
path = "~/.ssh/config"

[[inventory]]
type = "ansible"
path = "~/infra/inventories/*/hosts*"

[[ip_labels]]
pattern = "10.0.*"
label   = "Home Lab"

[[ip_labels]]
pattern = "10.10.*"
label   = "Office VPN"
```

See [`config.example.toml`](config.example.toml) for all options.

---

## Themes

```toml
# dark
theme = "tokyo-night"       # also: default · catppuccin · dracula · gruvbox · nord
# light
theme = "solarized-light"   # also: catppuccin-latte · rose-pine-dawn
```

<table>
<tr>
<td><img src="assets/theme-tokyo-night.png"/></td>
<td><img src="assets/theme-catppuccin.png"/></td>
<td><img src="assets/theme-rose-pine-dawn.png"/></td>
</tr>
</table>

[→ All 9 themes](assets/themes-preview.png)

---

## Uninstall

```bash
rm ~/.local/bin/sojourn
rm -rf ~/.config/sojourn/
```

---

MIT — see [LICENSE](LICENSE)
