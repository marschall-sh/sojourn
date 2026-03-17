# Teleport (tsh) Integration

sojourn can connect to [Teleport](https://goteleport.com/)-managed hosts alongside your regular SSH hosts. Teleport hosts appear in the same list, support fuzzy search, and connect via `tsh ssh` automatically.

---

## Requirements

- [`tsh`](https://goteleport.com/docs/connect-your-client/tsh/) installed and available (commonly at `/usr/local/bin/tsh` or via Homebrew)
- An active Teleport cluster you can log in to

---

## Setup

Run `sojourn setup` and proceed to the **Teleport** step:

1. Toggle **Enable Teleport** on with `Space`
2. Enter your **Cluster address** (e.g. `teleport.example.com:443`) — used with `tsh login --proxy=`
3. Optionally enter a **tsh login identity** (`tsh login --user=`) if your Teleport username differs from your system username
4. Leave the **tsh binary** field blank to use the auto-detected path, or enter a custom path
5. Tab to **Test connection** and press `Enter` — sojourn runs `tsh login` (if needed) and `tsh ls` to verify everything works and shows the host count
6. Press `Enter` on **Save & continue** to write the config

The Teleport section in your `~/.config/sojourn/config.toml` will look like:

```toml
[teleport]
enabled = true
proxy   = "teleport.example.com:443"
# username = "mario"   # optional: only if your Teleport identity differs from system user
# tsh_binary = "/usr/local/bin/tsh"   # optional: override auto-detected path
```

---

## How it works

**Inventory** — On startup sojourn runs `tsh ls --format=json` and adds the results to your host list. Teleport hosts show **Teleport** in the Location column and detail pane.

**Session check** — Before connecting, sojourn checks `tsh status`. If your session has expired it prints a prompt and runs `tsh login` so you can re-authenticate before the connection continues.

**Login picker** — Teleport certificates carry a list of allowed SSH logins (e.g. `root`, `doc`). When you press `Enter` on a Teleport host, sojourn reads these from `tsh status → Logins` and shows a small picker if there is more than one option. Select with `↑↓`, confirm with `Enter`, or cancel with `Esc`.

> **Note:** `teleport.username` is only used for `tsh login --user=` (your Teleport identity). It has no effect on which SSH login is used to connect — that comes from the `tsh status` login list.

---

## Source filter

Press `f` in the main list to cycle through:

- **All** — SSH and Teleport hosts together
- **SSH** — SSH hosts only
- **Teleport** — Teleport hosts only

The active filter is shown in the status bar as `[ssh]` or `[teleport]`.

---

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| No Teleport hosts appear | Run `tsh ls` manually to confirm you're logged in. Re-run `sojourn setup` to test the connection. |
| `ERROR: access denied` on connect | The SSH login you selected may not be allowed for that host. Check `tsh status` for the `Logins:` list. |
| Session expired on every launch | Your certificate TTL is short. sojourn will prompt to re-authenticate automatically. |
| `tsh` not found | Set `tsh_binary` in the config to the full path, or re-run `sojourn setup` to detect it. |
