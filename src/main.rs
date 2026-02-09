mod tcc;

#[cfg(test)]
use clap::CommandFactory;
#[cfg(test)]
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::process;

use tcc::{DbTarget, SERVICE_MAP, TccDb, TccEntry, auth_value_display, compact_client};

#[derive(Parser, Debug)]
#[command(name = "tcc", about = "Manage macOS TCC permissions", version)]
struct Cli {
    /// Operate on user DB instead of system DB
    #[arg(short, long, global = true)]
    user: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
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

    let hdr_svc = "SERVICE";
    let hdr_client = "CLIENT";
    let hdr_status = "STATUS";
    let hdr_source = "SOURCE";
    let hdr_modified = "LAST MODIFIED";

    let svc_w = entries
        .iter()
        .map(|e| e.service_display.len())
        .max()
        .unwrap_or(0)
        .max(hdr_svc.len());
    let client_w = display_clients
        .iter()
        .map(|c| c.len())
        .max()
        .unwrap_or(0)
        .max(hdr_client.len());
    let status_w = entries
        .iter()
        .map(|e| auth_value_display(e.auth_value).len())
        .max()
        .unwrap_or(0)
        .max(hdr_status.len());
    let source_w = hdr_source.len();
    let modified_w = entries
        .iter()
        .map(|e| e.last_modified.len())
        .max()
        .unwrap_or(0)
        .max(hdr_modified.len());

    println!(
        "{:<sw$}  {:<cw$}  {:<stw$}  {:<srw$}  {}",
        hdr_svc,
        hdr_client,
        hdr_status,
        hdr_source,
        hdr_modified,
        sw = svc_w,
        cw = client_w,
        stw = status_w,
        srw = source_w,
    );
    println!(
        "{}  {}  {}  {}  {}",
        "─".repeat(svc_w),
        "─".repeat(client_w),
        "─".repeat(status_w),
        "─".repeat(source_w),
        "─".repeat(modified_w),
    );

    let mut prev_client: Option<&str> = None;
    for (entry, display_client) in entries.iter().zip(display_clients.iter()) {
        let status_plain = auth_value_display(entry.auth_value);
        let status_colored = match entry.auth_value {
            0 => status_plain.red().to_string(),
            2 => status_plain.green().to_string(),
            3 => status_plain.yellow().to_string(),
            _ => status_plain.clone(),
        };
        // Pad based on visible length, then append the invisible ANSI tail
        let status_pad = status_w.saturating_sub(status_plain.len());
        let status_cell = format!("{}{}", status_colored, " ".repeat(status_pad));

        let client_cell = if prev_client == Some(display_client.as_str()) {
            "\u{2033}".to_string() // ″ double prime (ditto mark)
        } else {
            display_client.clone()
        };
        prev_client = Some(display_client.as_str());

        let source = if entry.is_system { "system" } else { "user" };

        println!(
            "{:<sw$}  {:<cw$}  {}  {:<srw$}  {}",
            entry.service_display,
            client_cell,
            status_cell,
            source,
            entry.last_modified,
            sw = svc_w,
            cw = client_w,
            srw = source_w,
        );
    }

    println!("\n{} entries total", entries.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(args)
    }

    // ── Subcommand parsing ─────────────────────────────────────────

    #[test]
    fn parse_list_no_flags() {
        let cli = parse(&["tcc", "list"]).unwrap();
        assert!(matches!(cli.command, Commands::List { .. }));
        assert!(!cli.user);
    }

    #[test]
    fn parse_list_with_client_and_service_filter() {
        let cli = parse(&["tcc", "list", "--client", "apple", "--service", "Camera"]).unwrap();
        match cli.command {
            Commands::List {
                client,
                service,
                compact,
            } => {
                assert_eq!(client.as_deref(), Some("apple"));
                assert_eq!(service.as_deref(), Some("Camera"));
                assert!(!compact);
            }
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn parse_list_compact() {
        let cli = parse(&["tcc", "list", "-c"]).unwrap();
        match cli.command {
            Commands::List { compact, .. } => assert!(compact),
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn parse_services() {
        let cli = parse(&["tcc", "services"]).unwrap();
        assert!(matches!(cli.command, Commands::Services));
    }

    #[test]
    fn parse_info() {
        let cli = parse(&["tcc", "info"]).unwrap();
        assert!(matches!(cli.command, Commands::Info));
    }

    #[test]
    fn parse_grant() {
        let cli = parse(&["tcc", "grant", "Camera", "com.app.test"]).unwrap();
        match cli.command {
            Commands::Grant {
                service,
                client_path,
            } => {
                assert_eq!(service, "Camera");
                assert_eq!(client_path, "com.app.test");
            }
            _ => panic!("expected Grant"),
        }
    }

    #[test]
    fn parse_revoke() {
        let cli = parse(&["tcc", "revoke", "Camera", "com.app.test"]).unwrap();
        match cli.command {
            Commands::Revoke {
                service,
                client_path,
            } => {
                assert_eq!(service, "Camera");
                assert_eq!(client_path, "com.app.test");
            }
            _ => panic!("expected Revoke"),
        }
    }

    #[test]
    fn parse_enable() {
        let cli = parse(&["tcc", "enable", "Accessibility", "/usr/bin/foo"]).unwrap();
        match cli.command {
            Commands::Enable {
                service,
                client_path,
            } => {
                assert_eq!(service, "Accessibility");
                assert_eq!(client_path, "/usr/bin/foo");
            }
            _ => panic!("expected Enable"),
        }
    }

    #[test]
    fn parse_disable() {
        let cli = parse(&["tcc", "disable", "Microphone", "com.app.x"]).unwrap();
        match cli.command {
            Commands::Disable {
                service,
                client_path,
            } => {
                assert_eq!(service, "Microphone");
                assert_eq!(client_path, "com.app.x");
            }
            _ => panic!("expected Disable"),
        }
    }

    #[test]
    fn parse_reset_with_client() {
        let cli = parse(&["tcc", "reset", "Camera", "com.app.test"]).unwrap();
        match cli.command {
            Commands::Reset {
                service,
                client_path,
            } => {
                assert_eq!(service, "Camera");
                assert_eq!(client_path.as_deref(), Some("com.app.test"));
            }
            _ => panic!("expected Reset"),
        }
    }

    #[test]
    fn parse_reset_without_client() {
        let cli = parse(&["tcc", "reset", "Camera"]).unwrap();
        match cli.command {
            Commands::Reset {
                service,
                client_path,
            } => {
                assert_eq!(service, "Camera");
                assert!(client_path.is_none());
            }
            _ => panic!("expected Reset"),
        }
    }

    #[test]
    fn parse_user_flag_global() {
        let cli = parse(&["tcc", "--user", "list"]).unwrap();
        assert!(cli.user);
    }

    #[test]
    fn parse_user_flag_after_subcommand() {
        let cli = parse(&["tcc", "list", "--user"]).unwrap();
        assert!(cli.user);
    }

    // ── Error cases ────────────────────────────────────────────────

    #[test]
    fn parse_no_subcommand_is_error() {
        let err = parse(&["tcc"]).unwrap_err();
        assert_eq!(
            err.kind(),
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
    }

    #[test]
    fn parse_unknown_subcommand_is_error() {
        let err = parse(&["tcc", "foobar"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
    }

    #[test]
    fn parse_grant_missing_args_is_error() {
        let err = parse(&["tcc", "grant"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn cli_has_version() {
        let cmd = Cli::command();
        assert!(cmd.get_version().is_some());
    }
}

/// Run a TCC command and handle the result uniformly
fn run_command(result: Result<String, tcc::TccError>) {
    match result {
        Ok(msg) => println!("{}", msg.green()),
        Err(e) => {
            eprintln!("{}: {}", "Error".red().bold(), e);
            process::exit(1);
        }
    }
}

/// Create a TccDb or exit with an error
fn make_db(target: DbTarget) -> TccDb {
    match TccDb::new(target) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("{}: {}", "Error".red().bold(), e);
            process::exit(1);
        }
    }
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
            let db = make_db(target);
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
        } => run_command(make_db(target).grant(&service, &client_path)),
        Commands::Revoke {
            service,
            client_path,
        } => run_command(make_db(target).revoke(&service, &client_path)),
        Commands::Enable {
            service,
            client_path,
        } => run_command(make_db(target).enable(&service, &client_path)),
        Commands::Disable {
            service,
            client_path,
        } => run_command(make_db(target).disable(&service, &client_path)),
        Commands::Reset {
            service,
            client_path,
        } => run_command(make_db(target).reset(&service, client_path.as_deref())),
        Commands::Services => {
            println!("{:<35}  DESCRIPTION", "INTERNAL NAME");
            println!("{:<35}  {}", "─".repeat(35), "─".repeat(25));
            let mut pairs: Vec<_> = SERVICE_MAP.iter().collect();
            pairs.sort_by_key(|(_, desc)| *desc);
            for (key, desc) in pairs {
                println!("{:<35}  {}", key.dimmed(), desc);
            }
        }
        Commands::Info => {
            let db = make_db(target);
            for line in db.info() {
                println!("{}", line);
            }
        }
    }
}
