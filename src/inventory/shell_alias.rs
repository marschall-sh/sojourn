/// Parse SSH aliases from shell RC files (.zshrc, .bashrc, .bash_profile, etc.)
///
/// Recognises patterns like:
///   alias prod='ssh deploy@prod.example.com'
///   alias jump="ssh -p 2222 -i ~/.ssh/key user@10.0.0.1"
///   alias bastion='ssh -J jump.host user@internal.host'
///   alias mybox='ssh -l root 192.168.1.10'
use std::collections::HashMap;

use anyhow::Result;

use super::{Host, InventorySource};

pub struct ShellAliasInventory {
    pub path: String,
}

impl InventorySource for ShellAliasInventory {
    fn load(&self) -> Result<Vec<Host>> {
        let expanded = shellexpand::tilde(&self.path).to_string();
        let content = match std::fs::read_to_string(&expanded) {
            Ok(c) => c,
            Err(_) => return Ok(vec![]),
        };
        let source_label = shorten_path(&expanded);
        Ok(parse_aliases(&content, &source_label))
    }
}

// ─────────────────────────────────────────────────────────────────────────────

fn parse_aliases(content: &str, source: &str) -> Vec<Host> {
    let mut hosts = Vec::new();

    for raw_line in content.lines() {
        // Handle line continuations (alias foo='\n  ssh ...')
        let line = raw_line.trim();

        // Must start with `alias`
        let rest = match line.strip_prefix("alias").and_then(|r| {
            let r = r.trim_start();
            if r.is_empty() || r.starts_with('=') { None } else { Some(r) }
        }) {
            Some(r) => r,
            None => continue,
        };

        // Split alias_name=value
        let eq_pos = match rest.find('=') {
            Some(p) => p,
            None => continue,
        };
        let alias_name = rest[..eq_pos].trim();
        let value_raw = rest[eq_pos + 1..].trim();

        // Strip surrounding quotes
        let value = strip_quotes(value_raw);

        // Must look like an ssh invocation
        let ssh_args = match extract_ssh_command(value) {
            Some(a) => a,
            None => continue,
        };

        if let Some(host) = parse_ssh_args(alias_name, &ssh_args, source) {
            hosts.push(host);
        }
    }

    hosts
}

/// Remove a leading `ssh` (with possible path prefix) and return the rest of
/// the command split into tokens, honouring simple single/double-quoted args.
fn extract_ssh_command(value: &str) -> Option<Vec<String>> {
    let tokens = shell_tokenize(value);
    // Find `ssh` or an absolute path ending in `ssh`
    let ssh_pos = tokens.iter().position(|t| {
        t == "ssh"
            || t.ends_with("/ssh")
            // allow env-variable wrappers like "env SSH_OPTIONS=... ssh"
            || (t == "env" && tokens.get(tokens.iter().position(|x| x == t).unwrap() + 1)
                .map(|n| n.contains('=') || n == "ssh")
                .unwrap_or(false))
    })?;
    Some(tokens[ssh_pos + 1..].to_vec())
}

/// Parse ssh CLI tokens into a Host.
fn parse_ssh_args(alias: &str, args: &[String], source: &str) -> Option<Host> {
    let mut user: Option<String> = None;
    let mut port: Option<u16> = None;
    let mut identity: Option<String> = None;
    let mut jump: Option<String> = None;
    let mut hostname_raw: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        let tok = &args[i];
        match tok.as_str() {
            // flags that consume the next token
            "-p" => { port = args.get(i + 1).and_then(|v| v.parse().ok()); i += 2; }
            "-i" => { identity = args.get(i + 1).map(|v| shellexpand::tilde(v).to_string()); i += 2; }
            "-l" => { user = args.get(i + 1).cloned(); i += 2; }
            "-J" => { jump = args.get(i + 1).cloned(); i += 2; }
            // compound flags: -p2222, -i/path/to/key
            t if t.starts_with("-p") && t.len() > 2 => {
                port = t[2..].parse().ok(); i += 1;
            }
            t if t.starts_with("-i") && t.len() > 2 => {
                identity = Some(shellexpand::tilde(&t[2..]).to_string()); i += 1;
            }
            t if t.starts_with("-l") && t.len() > 2 => {
                user = Some(t[2..].to_string()); i += 1;
            }
            // single-char flags with no argument (skip safely)
            t if t.starts_with('-') && t.len() == 2 && "46AaCfGgKkMNnqsTtVvXxYy".contains(&t[1..2]) => {
                i += 1;
            }
            // flags that consume next token but we don't care about the value
            t if t.starts_with('-') && t.len() == 2 && "bDEeFILmNoOQRSWw".contains(&t[1..2]) => {
                i += 2;
            }
            // anything else that doesn't start with - is the destination
            t if !t.starts_with('-') => {
                if hostname_raw.is_none() {
                    hostname_raw = Some(t.to_string());
                }
                // skip [command] after hostname
                break;
            }
            // unknown flag — skip one token
            _ => { i += 1; }
        }
    }

    let dest = hostname_raw?;

    // dest may be user@host or just host
    let (resolved_user, host) = if let Some(at) = dest.find('@') {
        (Some(dest[..at].to_string()), dest[at + 1..].to_string())
    } else {
        (None, dest)
    };

    // Merge -l flag with @-syntax (@ takes priority)
    let final_user = resolved_user.or(user);

    // Skip obviously non-host tokens (local commands sometimes look like args)
    if host.is_empty() || host.starts_with('-') {
        return None;
    }

    Some(Host {
        hostname: alias.to_string(),
        ip: if is_ip_or_fqdn(&host) { Some(host) } else { None },
        user: final_user,
        port,
        identity_file: identity,
        jump_host: jump,
        groups: vec!["shell_alias".to_string()],
        source: source.to_string(),
        tags: HashMap::new(),
        label: None,
        alias: None,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Tokenizer — simple POSIX-ish, good enough for alias values
// ─────────────────────────────────────────────────────────────────────────────

fn shell_tokenize(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                // single-quoted: everything until closing '
                for c2 in chars.by_ref() {
                    if c2 == '\'' { break; }
                    current.push(c2);
                }
            }
            '"' => {
                // double-quoted: everything until closing "
                for c2 in chars.by_ref() {
                    if c2 == '"' { break; }
                    current.push(c2);
                }
            }
            '\\' => {
                if let Some(c2) = chars.next() {
                    current.push(c2);
                }
            }
            ' ' | '\t' => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn strip_quotes(s: &str) -> &str {
    if (s.starts_with('\'') && s.ends_with('\''))
        || (s.starts_with('"') && s.ends_with('"'))
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn is_ip_or_fqdn(s: &str) -> bool {
    // Quick heuristic: has a dot or is a valid-looking hostname
    s.contains('.') || s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

fn shorten_path(path: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let h = home.display().to_string();
        if path.starts_with(&h) {
            return format!("~{}", &path[h.len()..]);
        }
    }
    path.to_string()
}

// ─────────────────────────────────────────────────────────────────────────────

/// Return the shell RC files that exist on this system, in priority order.
pub fn shell_rc_files() -> Vec<std::path::PathBuf> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return vec![],
    };
    [
        ".zshrc",
        ".bashrc",
        ".bash_profile",
        ".bash_aliases",
        ".profile",
        ".config/fish/config.fish",
    ]
    .iter()
    .map(|f| home.join(f))
    .filter(|p| p.exists())
    .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn aliases(src: &str) -> Vec<Host> {
        parse_aliases(src, "test")
    }

    #[test]
    fn basic_user_at_host() {
        let h = &aliases("alias prod='ssh deploy@prod.example.com'")[0];
        assert_eq!(h.hostname, "prod");
        assert_eq!(h.user.as_deref(), Some("deploy"));
        assert_eq!(h.ip.as_deref(), Some("prod.example.com"));
    }

    #[test]
    fn port_and_identity() {
        let h = &aliases("alias box=\"ssh -p 2222 -i ~/.ssh/id_rsa root@10.0.0.1\"")[0];
        assert_eq!(h.port, Some(2222));
        assert!(h.identity_file.as_deref().map(|p| !p.contains('~')).unwrap_or(false));
        assert_eq!(h.user.as_deref(), Some("root"));
    }

    #[test]
    fn jump_host() {
        let h = &aliases("alias internal='ssh -J jump.example.com user@192.168.1.5'")[0];
        assert_eq!(h.jump_host.as_deref(), Some("jump.example.com"));
    }

    #[test]
    fn l_flag_user() {
        let h = &aliases("alias srv='ssh -l admin 10.10.10.1'")[0];
        assert_eq!(h.user.as_deref(), Some("admin"));
    }

    #[test]
    fn non_ssh_alias_ignored() {
        let hosts = aliases("alias ll='ls -la'\nalias g='git'");
        assert!(hosts.is_empty());
    }
}
