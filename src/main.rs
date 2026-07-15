mod tcc;

#[cfg(test)]
use clap::CommandFactory;
#[cfg(test)]
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::{env, process};

use tcc::{
    DbTarget, ListResult, ListWarning, SERVICE_MAP, TccDb, TccEntry, TccError, WriteResult,
    auth_value_display, compact_client,
};

#[derive(Parser, Debug)]
#[command(name = "tccutil-rs", about = "Manage macOS TCC permissions", version)]
struct Cli {
    /// Force the per-user TCC database for list/write commands
    #[arg(short, long, global = true)]
    user: bool,

    /// Emit machine-readable JSON output
    #[arg(short = 'j', long, global = true)]
    json: bool,

    /// Allow writes against an unrecognized TCC schema digest
    #[arg(long, global = true)]
    force: bool,

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
    fn parse_force_flag_global() {
        let cli = parse(&["tcc", "--force", "grant", "Camera", "com.app"]).unwrap();
        assert!(cli.force);
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

    // ── JSON helpers ──────────────────────────────────────────────────

    #[test]
    fn json_escape_basic_specials() {
        assert_eq!(json_escape("\""), "\\\"");
        assert_eq!(json_escape("\\"), "\\\\");
        assert_eq!(json_escape("\n"), "\\n");
        assert_eq!(json_escape("\r"), "\\r");
        assert_eq!(json_escape("\t"), "\\t");
        assert_eq!(json_escape("\u{08}"), "\\b");
        assert_eq!(json_escape("\u{0C}"), "\\f");
    }

    #[test]
    fn json_escape_passes_printable_ascii_through() {
        assert_eq!(json_escape("hello world"), "hello world");
        assert_eq!(json_escape("a/b:c-d_e.f"), "a/b:c-d_e.f");
    }

    #[test]
    fn json_escape_emits_unicode_for_other_control_chars() {
        // U+0001 isn't in the named-escape list; it should be \u-encoded.
        assert_eq!(json_escape("\u{01}"), "\\u0001");
    }

    #[test]
    fn json_string_wraps_with_quotes() {
        assert_eq!(json_string("hi"), "\"hi\"");
        assert_eq!(json_string("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn json_message_data_shape() {
        let result = WriteResult::ok("done".into());
        assert_eq!(
            json_message_data(&result),
            "{\"message\":\"done\",\"warnings\":[]}"
        );
        let with_nl = WriteResult::ok("with\nnewline".into());
        assert_eq!(
            json_message_data(&with_nl),
            "{\"message\":\"with\\nnewline\",\"warnings\":[]}"
        );
    }

    // ── error_kind covers every TccError variant ──────────────────────

    #[test]
    fn error_kind_maps_every_variant() {
        use std::path::PathBuf;
        let cases: &[(TccError, &str)] = &[
            (
                TccError::DbOpen {
                    path: PathBuf::from("/x"),
                    source: "s".into(),
                },
                "DbOpen",
            ),
            (
                TccError::NotFound {
                    service: "s".into(),
                    client: "c".into(),
                },
                "NotFound",
            ),
            (
                TccError::NeedsRoot {
                    message: "m".into(),
                },
                "NeedsRoot",
            ),
            (TccError::UnknownService("x".into()), "UnknownService"),
            (
                TccError::AmbiguousService {
                    input: "x".into(),
                    matches: vec!["a".into()],
                },
                "AmbiguousService",
            ),
            (TccError::QueryFailed("q".into()), "QueryFailed"),
            (TccError::SchemaInvalid("s".into()), "SchemaInvalid"),
            (TccError::HomeDirNotFound, "HomeDirNotFound"),
            (TccError::WriteFailed("w".into()), "WriteFailed"),
        ];
        for (err, expected) in cases {
            assert_eq!(error_kind(err), *expected, "wrong kind for {:?}", err);
        }
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

fn json_message_data(result: &WriteResult) -> String {
    let warnings_json = result
        .warnings
        .iter()
        .map(|w| {
            format!(
                "{{\"kind\":{},\"message\":{}}}",
                json_string(w.kind.as_str()),
                json_string(&w.message),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"message\":{},\"warnings\":[{}]}}",
        json_string(&result.message),
        warnings_json
    )
}

fn json_list_data(result: &ListResult, compact: bool) -> String {
    let mut entry_json = Vec::with_capacity(result.entries.len());
    for entry in &result.entries {
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
    let warnings_json = result
        .warnings
        .iter()
        .map(|w| {
            format!(
                "{{\"kind\":{},\"source\":{},\"message\":{}}}",
                json_string(w.kind.as_str()),
                json_string(&w.source),
                json_string(&w.message),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"count\":{},\"entries\":[{}],\"warnings\":[{}]}}",
        result.entries.len(),
        entry_json.join(","),
        warnings_json
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

fn make_db(target: DbTarget, force: bool) -> Result<TccDb, TccError> {
    let mut db = TccDb::new(target)?;
    db.set_force(force);
    Ok(db)
}

fn fail_command(command: &'static str, json_mode: bool, error: TccError) -> ! {
    if json_mode {
        emit_json_error(command, error_kind(&error), error.to_string());
    } else {
        eprintln!("{}: {}", "Error".red().bold(), error);
    }
    process::exit(1);
}

fn emit_write_warnings_human(result: &WriteResult) {
    for w in &result.warnings {
        eprintln!("{}: {}", "Warning".yellow().bold(), w.message);
    }
}

fn emit_list_warnings_human(warnings: &[ListWarning]) {
    for w in warnings {
        eprintln!("{}: {}", "Warning".yellow().bold(), w.message);
    }
}

fn run_write_command<F>(
    command: &'static str,
    target: DbTarget,
    json_mode: bool,
    force: bool,
    op: F,
) where
    F: FnOnce(&TccDb) -> Result<WriteResult, TccError>,
{
    let db = match make_db(target, force) {
        Ok(db) => db,
        Err(e) => fail_command(command, json_mode, e),
    };
    match op(&db) {
        Ok(result) => {
            if json_mode {
                emit_json_success(command, json_message_data(&result));
            } else {
                emit_write_warnings_human(&result);
                println!("{}", result.message.green());
            }
        }
        Err(e) => fail_command(command, json_mode, e),
    }
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
    let force = cli.force;

    match cli.command {
        Commands::List {
            client,
            service,
            compact,
        } => {
            let db = match make_db(target, force) {
                Ok(db) => db,
                Err(e) => fail_command("list", json_mode, e),
            };

            match db.list(client.as_deref(), service.as_deref()) {
                Ok(result) => {
                    if json_mode {
                        emit_json_success("list", json_list_data(&result, compact));
                    } else {
                        emit_list_warnings_human(&result.warnings);
                        print_entries(&result.entries, compact);
                    }
                }
                Err(e) => fail_command("list", json_mode, e),
            }
        }
        Commands::Grant {
            service,
            client_path,
        } => run_write_command("grant", target, json_mode, force, |db| {
            db.grant(&service, &client_path)
        }),
        Commands::Revoke {
            service,
            client_path,
        } => run_write_command("revoke", target, json_mode, force, |db| {
            db.revoke(&service, &client_path)
        }),
        Commands::Enable {
            service,
            client_path,
        } => run_write_command("enable", target, json_mode, force, |db| {
            db.enable(&service, &client_path)
        }),
        Commands::Disable {
            service,
            client_path,
        } => run_write_command("disable", target, json_mode, force, |db| {
            db.disable(&service, &client_path)
        }),
        Commands::Reset {
            service,
            client_path,
        } => run_write_command("reset", target, json_mode, force, |db| {
            db.reset(&service, client_path.as_deref())
        }),
        Commands::Services => {
            if json_mode {
                emit_json_success("services", json_services_data());
            } else {
                let mut pairs: Vec<_> = SERVICE_MAP.iter().collect();
                pairs.sort_by_key(|(_, desc)| *desc);

                let hdr_name = "INTERNAL NAME";
                let hdr_desc = "DESCRIPTION";
                let name_w = pairs
                    .iter()
                    .map(|(k, _)| k.len())
                    .max()
                    .unwrap_or(0)
                    .max(hdr_name.len());
                let desc_w = pairs
                    .iter()
                    .map(|(_, d)| d.len())
                    .max()
                    .unwrap_or(0)
                    .max(hdr_desc.len());

                println!("{:<nw$}  {}", hdr_name, hdr_desc, nw = name_w);
                println!("{}  {}", "─".repeat(name_w), "─".repeat(desc_w));
                for (key, desc) in pairs {
                    // Pad after coloring so ANSI escapes don't consume width.
                    let pad = name_w.saturating_sub(key.len());
                    println!("{}{}  {}", key.dimmed(), " ".repeat(pad), desc);
                }
            }
        }
        Commands::Info => {
            let db = match make_db(target, force) {
                Ok(db) => db,
                Err(e) => fail_command("info", json_mode, e),
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
