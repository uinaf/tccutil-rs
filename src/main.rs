mod tcc;

use clap::{Parser, Subcommand};
use colored::Colorize;
use std::process;

use tcc::{compact_client, DbTarget, TccDb, TccEntry, SERVICE_MAP};

#[derive(Parser)]
#[command(name = "tcc", about = "Manage macOS TCC permissions", version)]
struct Cli {
    /// Operate on user DB instead of system DB
    #[arg(short, long, global = true)]
    user: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all TCC permissions
    List {
        /// Filter by client name (partial match)
        #[arg(long)]
        client: Option<String>,
        /// Filter by service name (partial match)
        #[arg(long)]
        service: Option<String>,
        /// Compact mode: show only binary name instead of full path
        #[arg(short, long)]
        compact: bool,
    },
    /// Grant a TCC permission (inserts new entry)
    Grant {
        /// Service name (e.g. Accessibility, Camera)
        service: String,
        /// Client bundle ID or path
        client_path: String,
    },
    /// Revoke a TCC permission (deletes entry)
    Revoke {
        /// Service name (e.g. Accessibility, Camera)
        service: String,
        /// Client bundle ID or path
        client_path: String,
    },
    /// Enable a TCC permission (set auth_value=2 for existing entry)
    Enable {
        /// Service name (e.g. Accessibility, Camera)
        service: String,
        /// Client bundle ID or path
        client_path: String,
    },
    /// Disable a TCC permission (set auth_value=0 for existing entry)
    Disable {
        /// Service name (e.g. Accessibility, Camera)
        service: String,
        /// Client bundle ID or path
        client_path: String,
    },
    /// Reset (delete) TCC entries for a service
    Reset {
        /// Service name (e.g. Accessibility, Camera)
        service: String,
        /// Optional: specific client to reset (if omitted, resets all entries for the service)
        client_path: Option<String>,
    },
    /// List all known TCC service names
    Services,
    /// Show TCC database info, macOS version, and SIP status
    Info,
}

fn print_entries(entries: &[TccEntry], compact: bool) {
    if entries.is_empty() {
        println!("{}", "No entries found.".dimmed());
        return;
    }

    let display_clients: Vec<String> = if compact {
        entries.iter().map(|e| compact_client(&e.client)).collect()
    } else {
        entries.iter().map(|e| e.client.clone()).collect()
    };

    let svc_width = entries
        .iter()
        .map(|e| e.service_display.len())
        .max()
        .unwrap_or(10)
        .max(10);
    let client_width = display_clients
        .iter()
        .map(|c| c.len())
        .max()
        .unwrap_or(10)
        .max(10);

    println!(
        "{:<sw$}  {:<cw$}  {:<10}  {:<8}  {}",
        "SERVICE",
        "CLIENT",
        "STATUS",
        "SOURCE",
        "LAST MODIFIED",
        sw = svc_width,
        cw = client_width,
    );
    println!(
        "{:<sw$}  {:<cw$}  {:<10}  {:<8}  {}",
        "─".repeat(svc_width),
        "─".repeat(client_width),
        "──────────",
        "────────",
        "─────────────",
        sw = svc_width,
        cw = client_width,
    );

    for (entry, display_client) in entries.iter().zip(display_clients.iter()) {
        let status_str = match entry.auth_value {
            0 => "denied".red().to_string(),
            2 => "granted".green().to_string(),
            3 => "limited".yellow().to_string(),
            v => format!("unknown({})", v),
        };

        let source = if entry.is_system { "system" } else { "user" };
        let modified = &entry.last_modified;

        println!(
            "{:<sw$}  {:<cw$}  {:<10}  {:<8}  {}",
            entry.service_display,
            display_client,
            status_str,
            source,
            modified,
            sw = svc_width,
            cw = client_width,
        );
    }

    println!("\n{} entries total", entries.len());
}

fn main() {
    let cli = Cli::parse();
    let target = if cli.user {
        DbTarget::User
    } else {
        DbTarget::Default
    };

    match cli.command {
        Commands::List {
            client,
            service,
            compact,
        } => {
            let db = TccDb::new(target);
            match db.list(client.as_deref(), service.as_deref()) {
                Ok(entries) => print_entries(&entries, compact),
                Err(e) => {
                    eprintln!("{}: {}", "Error".red().bold(), e);
                    process::exit(1);
                }
            }
        }
        Commands::Grant {
            service,
            client_path,
        } => {
            let db = TccDb::new(target);
            match db.grant(&service, &client_path) {
                Ok(msg) => println!("{}", msg.green()),
                Err(e) => {
                    eprintln!("{}: {}", "Error".red().bold(), e);
                    process::exit(1);
                }
            }
        }
        Commands::Revoke {
            service,
            client_path,
        } => {
            let db = TccDb::new(target);
            match db.revoke(&service, &client_path) {
                Ok(msg) => println!("{}", msg.green()),
                Err(e) => {
                    eprintln!("{}: {}", "Error".red().bold(), e);
                    process::exit(1);
                }
            }
        }
        Commands::Enable {
            service,
            client_path,
        } => {
            let db = TccDb::new(target);
            match db.enable(&service, &client_path) {
                Ok(msg) => println!("{}", msg.green()),
                Err(e) => {
                    eprintln!("{}: {}", "Error".red().bold(), e);
                    process::exit(1);
                }
            }
        }
        Commands::Disable {
            service,
            client_path,
        } => {
            let db = TccDb::new(target);
            match db.disable(&service, &client_path) {
                Ok(msg) => println!("{}", msg.green()),
                Err(e) => {
                    eprintln!("{}: {}", "Error".red().bold(), e);
                    process::exit(1);
                }
            }
        }
        Commands::Reset {
            service,
            client_path,
        } => {
            let db = TccDb::new(target);
            match db.reset(&service, client_path.as_deref()) {
                Ok(msg) => println!("{}", msg.green()),
                Err(e) => {
                    eprintln!("{}: {}", "Error".red().bold(), e);
                    process::exit(1);
                }
            }
        }
        Commands::Services => {
            println!("{:<35}  {}", "INTERNAL NAME", "DESCRIPTION");
            println!("{:<35}  {}", "─".repeat(35), "─".repeat(25));
            let mut pairs: Vec<_> = SERVICE_MAP.iter().collect();
            pairs.sort_by_key(|(_, desc)| *desc);
            for (key, desc) in pairs {
                println!("{:<35}  {}", key.dimmed(), desc);
            }
        }
        Commands::Info => {
            for line in TccDb::info() {
                println!("{}", line);
            }
        }
    }
}
