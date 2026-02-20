mod tcc;

#[cfg(test)]
use clap::CommandFactory;
#[cfg(test)]
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::{env, process};

use tcc::{DbTarget, SERVICE_MAP, TccDb, TccEntry, TccError, auth_value_display, compact_client};

#[derive(Parser, Debug)]
#[command(name = "tccutil-rs", about = "Manage macOS TCC permissions", version)]
struct Cli {
    /// Operate on user DB instead of system DB
    #[arg(short, long, global = true)]
    user: bool,

    /// Emit machine-readable JSON output
    #[arg(short = 'j', long, global = true)]
    json: bool,

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
        let status_pad = status_w.saturating_sub(status_plain.len());
        let status_cell = format!("{}{}", status_colored, " ".repeat(status_pad));

        let client_cell = if prev_client == Some(display_client.as_str()) {
            "\u{2033}".to_string()
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

    #[test]
    fn parse_list_no_flags() {
        let cli = parse(&["tcc", "list"]).unwrap();
        assert!(matches!(cli.command, Commands::List { .. }));
        assert!(!cli.user);
        assert!(!cli.json);
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

    #[test]
    fn parse_json_flag_global() {
        let cli = parse(&["tcc", "--json", "services"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn parse_json_flag_after_subcommand() {
        let cli = parse(&["tcc", "services", "--json"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn parse_json_short_flag() {
        let cli = parse(&["tcc", "-j", "info"]).unwrap();
        assert!(cli.json);
    }

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

fn error_kind(error: &TccError) -> &'static str {
    match error {
        TccError::DbOpen { .. } => "DbOpen",
        TccError::NotFound { .. } => "NotFound",
        TccError::NeedsRoot { .. } => "NeedsRoot",
        TccError::UnknownService(_) => "UnknownService",
        TccError::AmbiguousService { .. } => "AmbiguousService",
        TccError::QueryFailed(_) => "QueryFailed",
        TccError::SchemaInvalid(_) => "SchemaInvalid",
        TccError::HomeDirNotFound => "HomeDirNotFound",
        TccError::WriteFailed(_) => "WriteFailed",
    }
}

fn json_escape(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0C}' => escaped.push_str("\\f"),
            c if c.is_control() => escaped.push_str(&format!("\\u{:04x}", c as u32)),
            c => escaped.push(c),
        }
    }
    escaped
}

fn json_string(value: &str) -> String {
    format!("\"{}\"", json_escape(value))
}

fn emit_json(raw_json: String) {
    println!("{}", raw_json);
}

fn emit_json_success(command: &'static str, data_json: String) {
    emit_json(format!(
        "{{\"ok\":true,\"command\":{},\"data\":{},\"error\":null}}",
        json_string(command),
        data_json
    ));
}

fn emit_json_error(command: &'static str, kind: &'static str, message: String) {
    emit_json(format!(
        "{{\"ok\":false,\"command\":{},\"data\":null,\"error\":{{\"kind\":{},\"message\":{}}}}}",
        json_string(command),
        json_string(kind),
        json_string(&message),
    ));
}

fn json_message_data(message: &str) -> String {
    format!("{{\"message\":{}}}", json_string(message))
}

fn json_list_data(entries: &[TccEntry], compact: bool) -> String {
    let mut entry_json = Vec::with_capacity(entries.len());
    for entry in entries {
        let client = if compact {
            compact_client(&entry.client)
        } else {
            entry.client.clone()
        };
        let source = if entry.is_system { "system" } else { "user" };
        entry_json.push(format!(
            "{{\"service\":{},\"service_raw\":{},\"client\":{},\"status\":{},\"auth_value\":{},\"source\":{},\"last_modified\":{}}}",
            json_string(&entry.service_display),
            json_string(&entry.service_raw),
            json_string(&client),
            json_string(&auth_value_display(entry.auth_value)),
            entry.auth_value,
            json_string(source),
            json_string(&entry.last_modified),
        ));
    }
    format!(
        "{{\"count\":{},\"entries\":[{}]}}",
        entries.len(),
        entry_json.join(",")
    )
}

fn json_services_data() -> String {
    let mut pairs: Vec<_> = SERVICE_MAP.iter().collect();
    pairs.sort_by_key(|(_, desc)| *desc);
    let services = pairs
        .iter()
        .map(|(key, desc)| {
            format!(
                "{{\"internal_name\":{},\"description\":{}}}",
                json_string(key),
                json_string(desc),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"services\":[{}]}}", services)
}

fn json_info_data(lines: &[String]) -> String {
    let lines_json = lines
        .iter()
        .map(|line| json_string(line))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"lines\":[{}]}}", lines_json)
}

fn run_command(result: Result<String, TccError>) {
    match result {
        Ok(msg) => println!("{}", msg.green()),
        Err(e) => {
            eprintln!("{}: {}", "Error".red().bold(), e);
            process::exit(1);
        }
    }
}

fn make_db(target: DbTarget, suppress_warnings: bool) -> Result<TccDb, TccError> {
    let mut db = TccDb::new(target)?;
    db.set_suppress_warnings(suppress_warnings);
    Ok(db)
}

fn wants_json_from_args() -> bool {
    env::args().any(|arg| arg == "--json" || arg == "-j")
}

fn main() {
    let json_requested = wants_json_from_args();
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            if json_requested {
                emit_json_error("parse", "ParseError", err.to_string());
                process::exit(1);
            }
            err.exit();
        }
    };

    let target = if cli.user {
        DbTarget::User
    } else {
        DbTarget::Default
    };
    let json_mode = cli.json;

    match cli.command {
        Commands::List {
            client,
            service,
            compact,
        } => {
            let db = match make_db(target, json_mode) {
                Ok(db) => db,
                Err(e) => {
                    if json_mode {
                        emit_json_error("list", error_kind(&e), e.to_string());
                    } else {
                        eprintln!("{}: {}", "Error".red().bold(), e);
                    }
                    process::exit(1);
                }
            };

            match db.list(client.as_deref(), service.as_deref()) {
                Ok(entries) => {
                    if json_mode {
                        emit_json_success("list", json_list_data(&entries, compact));
                    } else {
                        print_entries(&entries, compact);
                    }
                }
                Err(e) => {
                    if json_mode {
                        emit_json_error("list", error_kind(&e), e.to_string());
                    } else {
                        eprintln!("{}: {}", "Error".red().bold(), e);
                    }
                    process::exit(1);
                }
            }
        }
        Commands::Grant {
            service,
            client_path,
        } => {
            let db = match make_db(target, json_mode) {
                Ok(db) => db,
                Err(e) => {
                    if json_mode {
                        emit_json_error("grant", error_kind(&e), e.to_string());
                    } else {
                        eprintln!("{}: {}", "Error".red().bold(), e);
                    }
                    process::exit(1);
                }
            };
            let result = db.grant(&service, &client_path);
            if json_mode {
                match result {
                    Ok(message) => emit_json_success("grant", json_message_data(&message)),
                    Err(e) => {
                        emit_json_error("grant", error_kind(&e), e.to_string());
                        process::exit(1);
                    }
                }
            } else {
                run_command(result);
            }
        }
        Commands::Revoke {
            service,
            client_path,
        } => {
            let db = match make_db(target, json_mode) {
                Ok(db) => db,
                Err(e) => {
                    if json_mode {
                        emit_json_error("revoke", error_kind(&e), e.to_string());
                    } else {
                        eprintln!("{}: {}", "Error".red().bold(), e);
                    }
                    process::exit(1);
                }
            };
            let result = db.revoke(&service, &client_path);
            if json_mode {
                match result {
                    Ok(message) => emit_json_success("revoke", json_message_data(&message)),
                    Err(e) => {
                        emit_json_error("revoke", error_kind(&e), e.to_string());
                        process::exit(1);
                    }
                }
            } else {
                run_command(result);
            }
        }
        Commands::Enable {
            service,
            client_path,
        } => {
            let db = match make_db(target, json_mode) {
                Ok(db) => db,
                Err(e) => {
                    if json_mode {
                        emit_json_error("enable", error_kind(&e), e.to_string());
                    } else {
                        eprintln!("{}: {}", "Error".red().bold(), e);
                    }
                    process::exit(1);
                }
            };
            let result = db.enable(&service, &client_path);
            if json_mode {
                match result {
                    Ok(message) => emit_json_success("enable", json_message_data(&message)),
                    Err(e) => {
                        emit_json_error("enable", error_kind(&e), e.to_string());
                        process::exit(1);
                    }
                }
            } else {
                run_command(result);
            }
        }
        Commands::Disable {
            service,
            client_path,
        } => {
            let db = match make_db(target, json_mode) {
                Ok(db) => db,
                Err(e) => {
                    if json_mode {
                        emit_json_error("disable", error_kind(&e), e.to_string());
                    } else {
                        eprintln!("{}: {}", "Error".red().bold(), e);
                    }
                    process::exit(1);
                }
            };
            let result = db.disable(&service, &client_path);
            if json_mode {
                match result {
                    Ok(message) => emit_json_success("disable", json_message_data(&message)),
                    Err(e) => {
                        emit_json_error("disable", error_kind(&e), e.to_string());
                        process::exit(1);
                    }
                }
            } else {
                run_command(result);
            }
        }
        Commands::Reset {
            service,
            client_path,
        } => {
            let db = match make_db(target, json_mode) {
                Ok(db) => db,
                Err(e) => {
                    if json_mode {
                        emit_json_error("reset", error_kind(&e), e.to_string());
                    } else {
                        eprintln!("{}: {}", "Error".red().bold(), e);
                    }
                    process::exit(1);
                }
            };
            let result = db.reset(&service, client_path.as_deref());
            if json_mode {
                match result {
                    Ok(message) => emit_json_success("reset", json_message_data(&message)),
                    Err(e) => {
                        emit_json_error("reset", error_kind(&e), e.to_string());
                        process::exit(1);
                    }
                }
            } else {
                run_command(result);
            }
        }
        Commands::Services => {
            if json_mode {
                emit_json_success("services", json_services_data());
            } else {
                println!("{:<35}  DESCRIPTION", "INTERNAL NAME");
                println!("{:<35}  {}", "─".repeat(35), "─".repeat(25));
                let mut pairs: Vec<_> = SERVICE_MAP.iter().collect();
                pairs.sort_by_key(|(_, desc)| *desc);
                for (key, desc) in pairs {
                    println!("{:<35}  {}", key.dimmed(), desc);
                }
            }
        }
        Commands::Info => {
            let db = match make_db(target, json_mode) {
                Ok(db) => db,
                Err(e) => {
                    if json_mode {
                        emit_json_error("info", error_kind(&e), e.to_string());
                    } else {
                        eprintln!("{}: {}", "Error".red().bold(), e);
                    }
                    process::exit(1);
                }
            };

            let lines = db.info();
            if json_mode {
                emit_json_success("info", json_info_data(&lines));
            } else {
                for line in lines {
                    println!("{}", line);
                }
            }
        }
    }
}
