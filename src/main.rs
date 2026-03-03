use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

mod app;
mod config;
mod discovery;
mod inventory;
mod search;
mod theme;
mod ui;
mod wizard;

use app::App;
use config::Config;
use wizard::{write_config, Wizard};

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "sojourn",
    about = "A beautiful TUI SSH host manager — find any host, connect instantly",
    version,
    after_help = "On first run sojourn automatically scans for inventory files and \
                  guides you through setup. Run `sojourn setup` at any time to \
                  reconfigure."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Initial search query (space-separated keywords)
    #[arg(value_name = "QUERY")]
    query: Vec<String>,

    /// Path to config file (overrides default ~/.config/sojourn/config.toml)
    #[arg(short, long, value_name = "FILE", global = true)]
    config: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Re-run the setup wizard (scan for inventories, edit IP labels)
    Setup,

    /// Quickly add an inventory source without running the full wizard
    ///
    /// Examples:
    ///   sojourn add ~/repos/ansible/inventories/prod/hosts_*
    ///   sojourn add ~/.config/sojourn/hosts.yaml
    Add {
        /// Path or glob pattern to the inventory file(s)
        #[arg(value_name = "PATH")]
        path: String,
    },

    /// Print the active config path and host counts per source
    #[command(name = "config-check")]
    ConfigCheck,
}

// ─────────────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Setup) => {
            run_wizard(true, cli.config.as_deref())?;
            Ok(())
        }
        Some(Commands::Add { path }) => cmd_add(path, cli.config.as_deref()),
        Some(Commands::ConfigCheck) => cmd_config_check(cli.config.as_deref()),
        None => {
            // First-run detection: no config file exists and user didn't pass --config
            let first_run = cli.config.is_none() && !config_file_exists();
            if first_run {
                run_wizard(false, None)?;
            }
            let initial_query = cli.query.join(" ");
            run_main_tui(cli.config.as_deref(), initial_query)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

fn run_wizard(force: bool, _config_path: Option<&str>) -> Result<()> {
    if force {
        println!("\x1b[1;36m⚡ sojourn setup\x1b[0m — re-running inventory wizard...\n");
    } else {
        println!("\x1b[1;36m⚡ Welcome to sojourn!\x1b[0m\n");
        println!("No config found — let's find your hosts.\n");
    }

    let wizard = Wizard::new();
    match wizard.run()? {
        Some(config) => {
            let saved_path = write_config(&config)?;
            println!(
                "\n\x1b[1;32m✓ Config saved to {}\x1b[0m",
                saved_path.display()
            );
            println!(
                "  {} inventory source(s)  ·  {} IP label(s)",
                config.inventory.len(),
                config.ip_labels.len()
            );
            if !force {
                println!("\n\x1b[2mLaunching sojourn...\x1b[0m\n");
            }
            Ok(())
        }
        None => {
            println!("\n\x1b[33mSetup cancelled. Run `sojourn setup` to try again.\x1b[0m");
            std::process::exit(0);
        }
    }
}

fn run_main_tui(config_path: Option<&str>, initial_query: String) -> Result<()> {
    let config = Config::load(config_path)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config, initial_query)?;
    let result = app.run(&mut terminal);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(ref e) = result {
        eprintln!("Error: {}", e);
    }
    result
}

fn cmd_add(path: String, config_path: Option<&str>) -> Result<()> {
    use config::InventoryConfig;
    use inventory::InventorySource;

    let expanded = shellexpand::tilde(&path).to_string();

    let (inv_config, count) = if expanded.ends_with(".yaml") || expanded.ends_with(".yml") {
        let inv = inventory::yaml_config::YamlInventory { path: expanded.clone() };
        let n = inv.load().unwrap_or_default().len();
        (InventoryConfig::Yaml { path: expanded }, n)
    } else {
        let inv = inventory::ansible::AnsibleInventory { pattern: expanded.clone() };
        let n = inv.load().unwrap_or_default().len();
        (InventoryConfig::Ansible { path: expanded }, n)
    };

    if count == 0 {
        eprintln!("⚠  No hosts found at '{}'. Check the path and try again.", path);
        std::process::exit(1);
    }

    let mut config = Config::load(config_path).unwrap_or_default();

    let new_path = match &inv_config {
        InventoryConfig::Ansible { path }
        | InventoryConfig::Yaml { path }
        | InventoryConfig::SshConfig { path }
        | InventoryConfig::ShellAlias { path } => path.clone(),
    };
    let already = config.inventory.iter().any(|c| {
        let p = match c {
            InventoryConfig::Ansible { path }
            | InventoryConfig::Yaml { path }
            | InventoryConfig::SshConfig { path }
            | InventoryConfig::ShellAlias { path } => path.as_str(),
        };
        p == new_path
    });

    if already {
        println!("ℹ  '{}' is already in your config.", path);
        return Ok(());
    }

    config.inventory.push(inv_config);
    let saved = write_config(&config)?;
    println!("✓  Added {} ({} hosts) — saved to {}", path, count, saved.display());
    Ok(())
}

fn cmd_config_check(config_path: Option<&str>) -> Result<()> {
    use config::InventoryConfig;
    use inventory::InventorySource;

    let path = config_path
        .map(String::from)
        .or_else(find_config_path)
        .unwrap_or_else(|| "~/.config/sojourn/config.toml (not found)".into());

    println!("\x1b[1;36msojourn config-check\x1b[0m");
    println!("Config: {}\n", path);

    let config = Config::load(config_path)?;
    let mut total = 0usize;

    for (i, src) in config.inventory.iter().enumerate() {
        let (label, count) = match src {
            InventoryConfig::Ansible { path } => {
                let n = inventory::ansible::AnsibleInventory { pattern: path.clone() }
                    .load().unwrap_or_default().len();
                (format!("[ansible]    {}", path), n)
            }
            InventoryConfig::SshConfig { path } => {
                let n = inventory::ssh_config::SshConfigInventory { path: path.clone() }
                    .load().unwrap_or_default().len();
                (format!("[ssh_config] {}", path), n)
            }
            InventoryConfig::Yaml { path } => {
                let n = inventory::yaml_config::YamlInventory { path: path.clone() }
                    .load().unwrap_or_default().len();
                (format!("[yaml]       {}", path), n)
            }
            InventoryConfig::ShellAlias { path } => {
                let n = inventory::shell_alias::ShellAliasInventory { path: path.clone() }
                    .load().unwrap_or_default().len();
                (format!("[shell_alias] {}", path), n)
            }
        };
        total += count;
        println!("  {}. {}  \x1b[32m({} hosts)\x1b[0m", i + 1, label, count);
    }

    println!("\n\x1b[1mTotal: {} hosts\x1b[0m", total);

    if !config.ip_labels.is_empty() {
        println!("\nIP labels ({}):", config.ip_labels.len());
        for l in &config.ip_labels {
            println!("  {:<20} → {}", l.pattern, l.label);
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────

fn config_file_exists() -> bool {
    find_config_path().is_some()
}

fn find_config_path() -> Option<String> {
    let home = dirs::home_dir()?;
    [
        home.join(".config/sojourn/config.toml"),
        home.join(".sojourn.toml"),
    ]
    .iter()
    .find(|p| p.exists())
    .map(|p| p.display().to_string())
}
