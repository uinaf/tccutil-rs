mod tcc;

use clap::{Parser, Subcommand};
use colored::Colorize;
use std::process;

use tcc::{TccDb, TccEntry, SERVICE_MAP};

#[derive(Parser)]
#[command(name = "tcc", about = "Manage macOS TCC permissions", version)]
struct Cli {
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
    },
    /// Grant a TCC permission
    Grant {
        /// Service name (e.g. Accessibility, Camera)
        service: String,
        /// Client bundle ID or path
        client_path: String,
    },
    /// Revoke a TCC permission
    Revoke {
        /// Service name (e.g. Accessibility, Camera)
        service: String,
        /// Client bundle ID or path
        client_path: String,
    },
    /// List all known TCC service names
    Services,
}

fn print_entries(entries: &[TccEntry]) {
    if entries.is_empty() {
        println!("{}", "No entries found.".dimmed());
        return;
    }

    let svc_width = entries.iter().map(|e| e.service_display.len()).max().unwrap_or(10).max(10);
    let client_width = entries.iter().map(|e| e.client.len()).max().unwrap_or(10).max(10);

    println!(
        "{:<sw$}  {:<cw$}  {:<10}  {:<8}  {}",
        "SERVICE", "CLIENT", "STATUS", "SOURCE", "LAST MODIFIED",
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

    for entry in entries {
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
            entry.client,
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

    match cli.command {
        Commands::List { client, service } => {
            let db = TccDb::new();
            match db.list(client.as_deref(), service.as_deref()) {
                Ok(entries) => print_entries(&entries),
                Err(e) => {
                    eprintln!("{}: {}", "Error".red().bold(), e);
                    process::exit(1);
                }
            }
        }
        Commands::Grant { service, client_path } => {
            let db = TccDb::new();
            match db.grant(&service, &client_path) {
                Ok(msg) => println!("{}", msg.green()),
                Err(e) => {
                    eprintln!("{}: {}", "Error".red().bold(), e);
                    process::exit(1);
                }
            }
        }
        Commands::Revoke { service, client_path } => {
            let db = TccDb::new();
            match db.revoke(&service, &client_path) {
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
    }
}
