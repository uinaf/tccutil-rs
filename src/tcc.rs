use chrono::{Local, TimeZone};
use rusqlite::{Connection, OpenFlags};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

pub static SERVICE_MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("kTCCServiceAccessibility", "Accessibility");
    m.insert("kTCCServiceScreenCapture", "Screen Recording");
    m.insert("kTCCServiceSystemPolicyAllFiles", "Full Disk Access");
    m.insert(
        "kTCCServiceSystemPolicySysAdminFiles",
        "Administer Computer (SysAdmin)",
    );
    m.insert("kTCCServiceSystemPolicyDesktopFolder", "Desktop Folder");
    m.insert("kTCCServiceSystemPolicyDocumentsFolder", "Documents Folder");
    m.insert("kTCCServiceSystemPolicyDownloadsFolder", "Downloads Folder");
    m.insert("kTCCServiceSystemPolicyNetworkVolumes", "Network Volumes");
    m.insert(
        "kTCCServiceSystemPolicyRemovableVolumes",
        "Removable Volumes",
    );
    m.insert("kTCCServiceSystemPolicyDeveloperFiles", "Developer Files");
    m.insert("kTCCServiceCamera", "Camera");
    m.insert("kTCCServiceMicrophone", "Microphone");
    m.insert("kTCCServicePhotos", "Photos");
    m.insert("kTCCServicePhotosAdd", "Photos (Add Only)");
    m.insert("kTCCServiceCalendar", "Calendar");
    m.insert("kTCCServiceContacts", "Contacts");
    m.insert("kTCCServiceReminders", "Reminders");
    m.insert("kTCCServiceLocation", "Location");
    m.insert("kTCCServiceAddressBook", "Address Book");
    m.insert("kTCCServiceMediaLibrary", "Media Library");
    m.insert("kTCCServiceAppleEvents", "Apple Events / Automation");
    m.insert("kTCCServiceListenEvent", "Input Monitoring");
    m.insert("kTCCServicePostEvent", "Post Events");
    m.insert("kTCCServiceSpeechRecognition", "Speech Recognition");
    m.insert("kTCCServiceBluetoothAlways", "Bluetooth");
    m.insert("kTCCServiceDeveloperTool", "Developer Tool");
    m.insert("kTCCServiceEndpointSecurityClient", "Endpoint Security");
    m.insert("kTCCServiceFileProviderDomain", "File Provider");
    m.insert("kTCCServiceFileProviderPresence", "File Provider Presence");
    m.insert("kTCCServiceFocusStatus", "Focus Status");
    m.insert("kTCCServiceLiverpool", "User Data (Liverpool)");
    m
});

/// Known schema digest hashes for the TCC access table, grouped by macOS version range.
/// Derived from tccutil.py's digest_check function.
const KNOWN_DIGESTS: &[&str] = &[
    "8e93d38f7c", // prior to El Capitan
    "9b2ea61b30", // El Capitan, Sierra, High Sierra
    "1072dc0e4b", // El Capitan, Sierra, High Sierra (alt)
    "ecc443615f", // Mojave, Catalina
    "80a4bb6912", // Mojave, Catalina (alt)
    "3d1c2a0e97", // Big Sur+
    "cef70648de", // Big Sur+ (alt)
    "34abf99d20", // Sonoma
    "e3a2181c14", // Sonoma (alt)
    "f773496775", // Sonoma (alt)
];

#[derive(Debug)]
pub enum TccError {
    DbOpen { path: PathBuf, source: String },
    NotFound { service: String, client: String },
    NeedsRoot { message: String },
    UnknownService(String),
    AmbiguousService { input: String, matches: Vec<String> },
    QueryFailed(String),
    SchemaInvalid(String),
    HomeDirNotFound,
    WriteFailed(String),
}

impl fmt::Display for TccError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TccError::DbOpen { path, source } => {
                write!(f, "Failed to open {}: {}", path.display(), source)?;
                if let Some(hint) = tcc_open_access_denied_hint(path, source) {
                    write!(f, "\n\n{}", hint)?;
                }
                Ok(())
            }
            TccError::NotFound { service, client } => {
                write!(
                    f,
                    "No entry found for service '{}' and client '{}'. \
                     Use `tccutil-rs grant` to insert a new entry.",
                    service, client
                )
            }
            TccError::NeedsRoot { message } => write!(f, "{}", message),
            TccError::UnknownService(s) => write!(
                f,
                "Unknown service '{}'. Run `tccutil-rs services` to see available services.",
                s
            ),
            TccError::AmbiguousService { input, matches } => write!(
                f,
                "Ambiguous service '{}'. Matches: {}",
                input,
                matches.join(", ")
            ),
            TccError::QueryFailed(s) => write!(f, "{}", s),
            TccError::SchemaInvalid(s) => write!(f, "{}", s),
            TccError::HomeDirNotFound => write!(f, "Cannot determine home directory"),
            TccError::WriteFailed(s) => write!(f, "{}", s),
        }
    }
}

fn tcc_open_access_denied_hint(path: &Path, source: &str) -> Option<String> {
    if !is_tcc_db_path(path) {
        return None;
    }

    let source_lower = source.to_lowercase();
    // SQLite on macOS reports "unable to open database file" when TCC blocks
    // a process without Full Disk Access, in addition to the explicit
    // "authorization denied" / "not authorized" surfaces seen on some versions.
    let is_open_denied = source_lower.contains("authorization denied")
        || source_lower.contains("open authorization denied")
        || source_lower.contains("not authorized")
        || source_lower.contains("unable to open database file");
    if !is_open_denied {
        return None;
    }

    Some(
        "macOS blocked access to TCC.db (Full Disk Access is required for terminal apps).\n\
         Grant Full Disk Access to the app launching this command (Terminal, iTerm, Ghostty, VS Code, etc.), then fully quit and reopen that app before retrying.\n\
         `sudo` does not bypass TCC privacy protections."
            .to_string(),
    )
}

fn is_tcc_db_path(path: &Path) -> bool {
    path == Path::new("/Library/Application Support/com.apple.TCC/TCC.db")
        || path.ends_with("Library/Application Support/com.apple.TCC/TCC.db")
}

#[derive(Debug)]
pub struct TccEntry {
    pub service_raw: String,
    pub service_display: String,
    pub client: String,
    pub auth_value: i32,
    pub last_modified: String,
    pub is_system: bool,
}

/// Machine-readable warning from a successful-but-incomplete `list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListWarning {
    pub kind: ListWarningKind,
    /// `"user"` or `"system"`.
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListWarningKind {
    DbUnreadable,
    MalformedRow,
}

impl ListWarningKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DbUnreadable => "db_unreadable",
            Self::MalformedRow => "malformed_row",
        }
    }
}

/// Result of a successful `list` that may still be incomplete.
#[derive(Debug)]
pub struct ListResult {
    pub entries: Vec<TccEntry>,
    pub warnings: Vec<ListWarning>,
}

/// Machine-readable warning from a successful write (`--force` schema, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteWarning {
    pub kind: WriteWarningKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteWarningKind {
    UnknownSchema,
}

impl WriteWarningKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnknownSchema => "unknown_schema",
        }
    }
}

/// Result of a successful mutating command.
#[derive(Debug)]
pub struct WriteResult {
    pub message: String,
    pub warnings: Vec<WriteWarning>,
}

impl WriteResult {
    pub fn ok(message: String) -> Self {
        Self {
            message,
            warnings: Vec::new(),
        }
    }

    fn with_schema_warning(message: String, warning: Option<String>) -> Self {
        let mut warnings = Vec::new();
        if let Some(message) = warning {
            warnings.push(WriteWarning {
                kind: WriteWarningKind::UnknownSchema,
                message,
            });
        }
        Self { message, warnings }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum DbTarget {
    /// Use both DBs for reads, system for writes (default)
    Default,
    /// User DB only
    User,
}

pub struct TccDb {
    user_db_path: PathBuf,
    system_db_path: PathBuf,
    target: DbTarget,
    /// When true, allow writes against an unrecognized access-table schema digest.
    force: bool,
}

impl TccDb {
    pub fn new(target: DbTarget) -> Result<Self, TccError> {
        let home = dirs::home_dir().ok_or(TccError::HomeDirNotFound)?;
        Ok(Self {
            user_db_path: home.join("Library/Application Support/com.apple.TCC/TCC.db"),
            system_db_path: PathBuf::from(Self::LIVE_SYSTEM_DB),
            target,
            force: false,
        })
    }

    #[cfg(test)]
    pub fn with_paths(user: PathBuf, system: PathBuf, target: DbTarget) -> Self {
        Self {
            user_db_path: user,
            system_db_path: system,
            target,
            force: false,
        }
    }

    pub fn set_force(&mut self, force: bool) {
        self.force = force;
    }

    pub(crate) fn format_timestamp(ts: i64) -> String {
        if ts == 0 {
            return "N/A".to_string();
        }
        // macOS TCC uses CoreData timestamps (seconds since 2001-01-01) or Unix timestamps.
        let unix_ts = if ts < 1_000_000_000 {
            ts + 978_307_200
        } else {
            ts
        };

        match Local.timestamp_opt(unix_ts, 0) {
            chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            _ => format!("{}", ts),
        }
    }

    pub(crate) fn service_display_name(raw: &str) -> String {
        SERVICE_MAP
            .get(raw)
            .map(|s| s.to_string())
            .unwrap_or_else(|| raw.strip_prefix("kTCCService").unwrap_or(raw).to_string())
    }

    fn read_db(
        path: &Path,
        source: &str,
        is_system: bool,
    ) -> Result<(Vec<TccEntry>, Vec<ListWarning>), TccError> {
        if !path.exists() {
            return Ok((vec![], vec![]));
        }

        let conn =
            Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| {
                TccError::DbOpen {
                    path: path.to_path_buf(),
                    source: e.to_string(),
                }
            })?;

        let query = "SELECT service, client, auth_value, \
                     COALESCE(last_modified, 0) as modified \
                     FROM access";

        let result = conn.prepare(query);
        let mut stmt = match result {
            Ok(s) => s,
            Err(_) => {
                let fallback = "SELECT service, client, auth_value, 0 as modified FROM access";
                conn.prepare(fallback).map_err(|e| {
                    TccError::QueryFailed(format!("Query failed on {}: {}", path.display(), e))
                })?
            }
        };

        let rows = stmt
            .query_map([], |row| {
                let service_raw: String = row.get(0)?;
                let client: String = row.get(1)?;
                let auth_value: i32 = row.get(2)?;
                let modified: i64 = row.get(3)?;

                Ok(TccEntry {
                    service_display: Self::service_display_name(&service_raw),
                    service_raw,
                    client,
                    auth_value,
                    last_modified: Self::format_timestamp(modified),
                    is_system,
                })
            })
            .map_err(|e| {
                TccError::QueryFailed(format!("Query error on {}: {}", path.display(), e))
            })?;

        let mut entries = Vec::new();
        let mut warnings = Vec::new();
        for result in rows {
            match result {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    warnings.push(ListWarning {
                        kind: ListWarningKind::MalformedRow,
                        source: source.to_string(),
                        message: format!("skipping malformed row in {}: {}", path.display(), e),
                    });
                }
            }
        }

        Ok((entries, warnings))
    }

    pub fn list(
        &self,
        client_filter: Option<&str>,
        service_filter: Option<&str>,
    ) -> Result<ListResult, TccError> {
        let mut entries = Vec::new();
        let mut attempted = 0usize;
        let mut errors: Vec<(String, TccError)> = Vec::new();
        let mut warnings: Vec<ListWarning> = Vec::new();

        if self.target == DbTarget::Default || self.target == DbTarget::User {
            attempted += 1;
            match Self::read_db(&self.user_db_path, "user", false) {
                Ok((mut e, mut row_warnings)) => {
                    entries.append(&mut e);
                    warnings.append(&mut row_warnings);
                }
                Err(e) => errors.push(("user".to_string(), e)),
            }
        }

        if self.target == DbTarget::Default {
            attempted += 1;
            match Self::read_db(&self.system_db_path, "system", true) {
                Ok((mut e, mut row_warnings)) => {
                    entries.append(&mut e);
                    warnings.append(&mut row_warnings);
                }
                Err(e) => errors.push(("system".to_string(), e)),
            }
        }

        if attempted > 0 && errors.len() == attempted {
            return Err(errors.into_iter().next().unwrap().1);
        }

        for (source, e) in errors {
            warnings.push(ListWarning {
                kind: ListWarningKind::DbUnreadable,
                source,
                message: e.to_string(),
            });
        }

        if let Some(cf) = client_filter {
            let cf_lower = cf.to_lowercase();
            entries.retain(|e| e.client.to_lowercase().contains(&cf_lower));
        }
        if let Some(sf) = service_filter {
            let sf_lower = sf.to_lowercase();
            entries.retain(|e| {
                e.service_display.to_lowercase().contains(&sf_lower)
                    || e.service_raw.to_lowercase().contains(&sf_lower)
            });
        }

        entries.sort_by(|a, b| {
            a.service_display
                .cmp(&b.service_display)
                .then(a.client.cmp(&b.client))
        });

        Ok(ListResult { entries, warnings })
    }

    pub fn resolve_service_name(&self, input: &str) -> Result<String, TccError> {
        if SERVICE_MAP.contains_key(input) {
            return Ok(input.to_string());
        }
        let input_lower = input.to_lowercase();
        // Exact display name match (case-insensitive)
        for (key, display) in SERVICE_MAP.iter() {
            if display.to_lowercase() == input_lower {
                return Ok(key.to_string());
            }
        }
        // Partial display name match — collect all, error if ambiguous
        let partial_matches: Vec<_> = SERVICE_MAP
            .iter()
            .filter(|(_, display)| display.to_lowercase().contains(&input_lower))
            .collect();
        match partial_matches.len() {
            0 => {}
            1 => return Ok(partial_matches[0].0.to_string()),
            _ => {
                let mut names: Vec<_> =
                    partial_matches.iter().map(|(_, d)| d.to_string()).collect();
                names.sort();
                return Err(TccError::AmbiguousService {
                    input: input.to_string(),
                    matches: names,
                });
            }
        }
        let prefixed = format!("kTCCService{}", input);
        if SERVICE_MAP.contains_key(prefixed.as_str()) {
            return Ok(prefixed);
        }
        Err(TccError::UnknownService(input.to_string()))
    }

    /// Services whose live grants live in the system TCC.db (not the per-user DB).
    /// Folder SystemPolicy* services (Desktop/Documents/…) stay in the user DB.
    fn is_system_service(service: &str) -> bool {
        matches!(
            service,
            "kTCCServiceAccessibility"
                | "kTCCServiceScreenCapture"
                | "kTCCServiceListenEvent"
                | "kTCCServicePostEvent"
                | "kTCCServiceEndpointSecurityClient"
                | "kTCCServiceDeveloperTool"
                | "kTCCServiceSystemPolicyAllFiles"
                | "kTCCServiceSystemPolicySysAdminFiles"
        )
    }

    /// Determine the target DB path for a write operation
    fn write_db_path(&self, service_key: &str) -> &Path {
        match self.target {
            DbTarget::User => &self.user_db_path,
            DbTarget::Default => {
                if Self::is_system_service(service_key) {
                    &self.system_db_path
                } else {
                    &self.user_db_path
                }
            }
        }
    }

    /// Live system TCC.db path — writes here always require root.
    pub const LIVE_SYSTEM_DB: &'static str = "/Library/Application Support/com.apple.TCC/TCC.db";

    fn path_requires_root(path: &Path) -> bool {
        path == Path::new(Self::LIVE_SYSTEM_DB) && !nix_is_root()
    }

    /// Check if root is needed and we don't have it
    fn check_root_for_write(
        &self,
        service_key: &str,
        action: &str,
        service_input: &str,
        client: &str,
    ) -> Result<(), TccError> {
        let db_path = self.write_db_path(service_key);
        if Self::path_requires_root(db_path) {
            return Err(TccError::NeedsRoot {
                message: format!(
                    "Service '{}' requires the system TCC database.\n\
                     Run with sudo: sudo tccutil-rs {} {} {}",
                    Self::service_display_name(service_key),
                    action,
                    service_input,
                    client
                ),
            });
        }
        Ok(())
    }

    /// Validate the DB schema before writing.
    /// Unknown digests fail closed unless `allow_unknown` is true (then a warning is returned).
    fn validate_schema(conn: &Connection, allow_unknown: bool) -> Result<Option<String>, TccError> {
        let sql: String = match conn.query_row(
            "SELECT sql FROM sqlite_master WHERE name='access' AND type='table'",
            [],
            |row| row.get(0),
        ) {
            Ok(sql) => sql,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(TccError::SchemaInvalid(
                    "Could not read TCC database schema. The access table may not exist."
                        .to_string(),
                ));
            }
            Err(e) => {
                return Err(TccError::QueryFailed(format!(
                    "Failed to read TCC database schema: {}",
                    e
                )));
            }
        };

        let mut hasher = sha1_smol::Sha1::new();
        hasher.update(sql.as_bytes());
        let hex = hasher.digest().to_string();
        let short = &hex[..10];

        if KNOWN_DIGESTS.contains(&short) {
            Ok(None)
        } else if allow_unknown {
            Ok(Some(format!(
                "Unknown TCC database schema (digest: {}). Proceeding because --force was set — results may vary.",
                short
            )))
        } else {
            Err(TccError::SchemaInvalid(format!(
                "Unknown TCC database schema (digest: {}). \
                 Refusing to write. Re-run with --force if you intentionally accept this risk, \
                 or update KNOWN_DIGESTS after verifying the new schema.",
                short
            )))
        }
    }

    /// Open a writable connection with schema validation
    fn open_writable(&self, service_key: &str) -> Result<(Connection, Option<String>), TccError> {
        let db_path = self.write_db_path(service_key);
        let conn = Connection::open(db_path).map_err(|e| TccError::DbOpen {
            path: db_path.to_path_buf(),
            source: e.to_string(),
        })?;
        let warning = Self::validate_schema(&conn, self.force)?;
        Ok((conn, warning))
    }

    pub fn grant(&self, service: &str, client: &str) -> Result<WriteResult, TccError> {
        let service_key = self.resolve_service_name(service)?;
        self.check_root_for_write(&service_key, "grant", service, client)?;

        let (conn, warning) = self.open_writable(&service_key)?;

        let client_type: i32 = if client.starts_with('/') { 0 } else { 1 };
        let now = chrono::Utc::now().timestamp() - 978_307_200;
        let sql = "INSERT OR REPLACE INTO access \
                   (service, client, client_type, auth_value, auth_reason, auth_version, flags, last_modified) \
                   VALUES (?1, ?2, ?3, 2, 0, 1, 0, ?4)";

        conn.execute(
            sql,
            rusqlite::params![service_key, client, client_type, now],
        )
        .map_err(|e| {
            TccError::WriteFailed(format!(
                "Failed to grant: {}. Note: SIP may prevent TCC.db writes on macOS 10.14+",
                e
            ))
        })?;

        Ok(WriteResult::with_schema_warning(
            format!(
                "Granted {} access for '{}'",
                Self::service_display_name(&service_key),
                client
            ),
            warning,
        ))
    }

    pub fn revoke(&self, service: &str, client: &str) -> Result<WriteResult, TccError> {
        let service_key = self.resolve_service_name(service)?;
        self.check_root_for_write(&service_key, "revoke", service, client)?;

        let (conn, warning) = self.open_writable(&service_key)?;

        let deleted = conn
            .execute(
                "DELETE FROM access WHERE service = ?1 AND client = ?2",
                rusqlite::params![service_key, client],
            )
            .map_err(|e| {
                TccError::WriteFailed(format!(
                    "Failed to revoke: {}. Note: SIP may prevent TCC.db writes.",
                    e
                ))
            })?;

        if deleted == 0 {
            Err(TccError::NotFound {
                service: Self::service_display_name(&service_key),
                client: client.to_string(),
            })
        } else {
            Ok(WriteResult::with_schema_warning(
                format!(
                    "Revoked {} access for '{}'",
                    Self::service_display_name(&service_key),
                    client
                ),
                warning,
            ))
        }
    }

    pub fn enable(&self, service: &str, client: &str) -> Result<WriteResult, TccError> {
        let service_key = self.resolve_service_name(service)?;
        self.check_root_for_write(&service_key, "enable", service, client)?;

        let (conn, warning) = self.open_writable(&service_key)?;

        let now = chrono::Utc::now().timestamp() - 978_307_200;
        let updated = conn
            .execute(
                "UPDATE access SET auth_value = 2, last_modified = ?3 WHERE service = ?1 AND client = ?2",
                rusqlite::params![service_key, client, now],
            )
            .map_err(|e| {
                TccError::WriteFailed(format!(
                    "Failed to enable: {}. Note: SIP may prevent TCC.db writes.",
                    e
                ))
            })?;

        if updated == 0 {
            Err(TccError::NotFound {
                service: Self::service_display_name(&service_key),
                client: client.to_string(),
            })
        } else {
            Ok(WriteResult::with_schema_warning(
                format!(
                    "Enabled {} access for '{}'",
                    Self::service_display_name(&service_key),
                    client
                ),
                warning,
            ))
        }
    }

    pub fn disable(&self, service: &str, client: &str) -> Result<WriteResult, TccError> {
        let service_key = self.resolve_service_name(service)?;
        self.check_root_for_write(&service_key, "disable", service, client)?;

        let (conn, warning) = self.open_writable(&service_key)?;

        let now = chrono::Utc::now().timestamp() - 978_307_200;
        let updated = conn
            .execute(
                "UPDATE access SET auth_value = 0, last_modified = ?3 WHERE service = ?1 AND client = ?2",
                rusqlite::params![service_key, client, now],
            )
            .map_err(|e| {
                TccError::WriteFailed(format!(
                    "Failed to disable: {}. Note: SIP may prevent TCC.db writes.",
                    e
                ))
            })?;

        if updated == 0 {
            Err(TccError::NotFound {
                service: Self::service_display_name(&service_key),
                client: client.to_string(),
            })
        } else {
            Ok(WriteResult::with_schema_warning(
                format!(
                    "Disabled {} access for '{}'",
                    Self::service_display_name(&service_key),
                    client
                ),
                warning,
            ))
        }
    }

    pub fn reset(&self, service: &str, client: Option<&str>) -> Result<WriteResult, TccError> {
        let service_key = self.resolve_service_name(service)?;

        if let Some(c) = client {
            self.check_root_for_write(&service_key, "reset", service, c)?;

            let (conn, warning) = self.open_writable(&service_key)?;

            let deleted = conn
                .execute(
                    "DELETE FROM access WHERE service = ?1 AND client = ?2",
                    rusqlite::params![service_key, c],
                )
                .map_err(|e| TccError::WriteFailed(format!("Failed to reset: {}", e)))?;

            if deleted == 0 {
                Err(TccError::NotFound {
                    service: Self::service_display_name(&service_key),
                    client: c.to_string(),
                })
            } else {
                Ok(WriteResult::with_schema_warning(
                    format!(
                        "Reset {} entry for '{}'",
                        Self::service_display_name(&service_key),
                        c
                    ),
                    warning,
                ))
            }
        } else {
            self.reset_all_for_service(&service_key, service)
        }
    }

    /// Reset every matching row for `service_key` across targeted DBs.
    /// When both DBs are mutated, uses ATTACH + one transaction so a
    /// mid-flight *statement* failure rolls back both deletes. SQLite does
    /// not guarantee crash-atomicity across ATTACHed databases in WAL mode
    /// (common for TCC.db); we do not claim power-loss atomicity.
    fn reset_all_for_service(
        &self,
        service_key: &str,
        service_input: &str,
    ) -> Result<WriteResult, TccError> {
        let paths: Vec<(&Path, &str)> = match self.target {
            DbTarget::User => vec![(&self.user_db_path, "user")],
            DbTarget::Default => vec![
                (&self.user_db_path, "user"),
                (&self.system_db_path, "system"),
            ],
        };

        let mut existing_paths: Vec<(&Path, &str)> = Vec::new();
        for &(db_path, label) in &paths {
            match db_path.try_exists() {
                Ok(true) => existing_paths.push((db_path, label)),
                Ok(false) => {}
                Err(e) => {
                    return Err(TccError::DbOpen {
                        path: db_path.to_path_buf(),
                        source: format!("cannot access path: {}", e),
                    });
                }
            }
        }

        let system_present = existing_paths
            .iter()
            .any(|(p, _)| *p == self.system_db_path.as_path());
        if system_present && Self::path_requires_root(&self.system_db_path) {
            let conn =
                Connection::open_with_flags(&self.system_db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                    .map_err(|e| {
                        let mut msg = e.to_string();
                        msg.push_str(
                    "\nIf you only need to reset the per-user database, re-run with --user.",
                );
                        TccError::DbOpen {
                            path: self.system_db_path.clone(),
                            source: msg,
                        }
                    })?;
            let system_has_rows: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM access WHERE service = ?1",
                    rusqlite::params![service_key],
                    |row| row.get(0),
                )
                .map_err(|e| {
                    TccError::QueryFailed(format!(
                        "Failed to inspect system TCC database before reset: {}. \
                         If you only need the per-user database, re-run with --user.",
                        e
                    ))
                })?;

            if system_has_rows > 0 {
                return Err(TccError::NeedsRoot {
                    message: format!(
                        "Resetting all '{}' entries requires the system TCC database.\n\
                         Run with sudo: sudo tccutil-rs reset {}\n\
                         Or reset only the user database: tccutil-rs --user reset {}",
                        Self::service_display_name(service_key),
                        service_input,
                        service_input
                    ),
                });
            }
        }

        // Paths we will actually mutate (skip live system DB when non-root and empty).
        let mut mutate: Vec<(&Path, &str)> = Vec::new();
        for (db_path, label) in existing_paths {
            if Self::path_requires_root(db_path) {
                // Confirmed zero matching system rows above; skip quietly.
                continue;
            }
            mutate.push((db_path, label));
        }

        if mutate.is_empty() {
            return Ok(WriteResult::ok(format!(
                "Reset all {} entries (0 deleted)",
                Self::service_display_name(service_key)
            )));
        }

        // Validate schemas up front (and collect --force warnings) before any DELETE.
        let mut schema_warnings: Vec<WriteWarning> = Vec::new();
        for (db_path, label) in &mutate {
            let conn = Connection::open(db_path).map_err(|e| TccError::DbOpen {
                path: db_path.to_path_buf(),
                source: e.to_string(),
            })?;
            match Self::validate_schema(&conn, self.force) {
                Err(TccError::QueryFailed(msg)) => {
                    return Err(TccError::QueryFailed(format!("{} DB: {}", label, msg)));
                }
                Err(e) => {
                    return Err(TccError::SchemaInvalid(format!("{} DB: {}", label, e)));
                }
                Ok(Some(message)) => {
                    schema_warnings.push(WriteWarning {
                        kind: WriteWarningKind::UnknownSchema,
                        message: format!("{} DB: {}", label, message),
                    });
                }
                Ok(None) => {}
            }
        }

        let total_deleted = if mutate.len() == 1 {
            let (db_path, label) = mutate[0];
            let conn = Connection::open(db_path).map_err(|e| TccError::DbOpen {
                path: db_path.to_path_buf(),
                source: e.to_string(),
            })?;
            conn.execute(
                "DELETE FROM access WHERE service = ?1",
                rusqlite::params![service_key],
            )
            .map_err(|e| TccError::WriteFailed(format!("Failed to reset {} DB: {}", label, e)))?
        } else {
            // Cross-DB delete in one transaction so a statement failure rolls
            // back both sides. (WAL crash-atomicity across ATTACH is not
            // guaranteed by SQLite; we only rely on statement-level rollback.)
            let (primary_path, primary_label) = mutate[0];
            let (secondary_path, secondary_label) = mutate[1];
            let conn = Connection::open(primary_path).map_err(|e| TccError::DbOpen {
                path: primary_path.to_path_buf(),
                source: e.to_string(),
            })?;
            conn.execute(
                "ATTACH DATABASE ?1 AS sys_tcc",
                rusqlite::params![secondary_path.to_string_lossy().as_ref()],
            )
            .map_err(|e| {
                TccError::WriteFailed(format!(
                    "Failed to attach {} DB for dual-DB reset: {}",
                    secondary_label, e
                ))
            })?;

            let delete_result = (|| -> Result<usize, TccError> {
                conn.execute_batch("BEGIN IMMEDIATE").map_err(|e| {
                    TccError::WriteFailed(format!("Failed to begin reset txn: {}", e))
                })?;
                let n_primary = conn
                    .execute(
                        "DELETE FROM main.access WHERE service = ?1",
                        rusqlite::params![service_key],
                    )
                    .map_err(|e| {
                        TccError::WriteFailed(format!(
                            "Failed to reset {} DB: {}",
                            primary_label, e
                        ))
                    })?;
                let n_secondary = conn
                    .execute(
                        "DELETE FROM sys_tcc.access WHERE service = ?1",
                        rusqlite::params![service_key],
                    )
                    .map_err(|e| {
                        TccError::WriteFailed(format!(
                            "Failed to reset {} DB: {}",
                            secondary_label, e
                        ))
                    })?;
                conn.execute_batch("COMMIT").map_err(|e| {
                    TccError::WriteFailed(format!("Failed to commit reset txn: {}", e))
                })?;
                Ok(n_primary + n_secondary)
            })();

            if delete_result.is_err() {
                let _ = conn.execute_batch("ROLLBACK");
            }
            let _ = conn.execute_batch("DETACH DATABASE sys_tcc");
            delete_result?
        };

        Ok(WriteResult {
            message: format!(
                "Reset all {} entries ({} deleted)",
                Self::service_display_name(service_key),
                total_deleted
            ),
            warnings: schema_warnings,
        })
    }

    pub fn info(&self) -> Vec<String> {
        let mut lines = Vec::new();

        // macOS version — use absolute path for defensive coding
        let macos_ver = Command::new("/usr/bin/sw_vers")
            .arg("-productVersion")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        lines.push(format!("macOS version: {}", macos_ver));

        // SIP status — use absolute path for defensive coding
        let sip = Command::new("/usr/bin/csrutil")
            .arg("status")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unknown (csrutil not available)".to_string());
        lines.push(format!("SIP status: {}", sip));

        lines.push(String::new());

        // DB info
        for (label, path) in [
            ("User DB", &self.user_db_path),
            ("System DB", &self.system_db_path),
        ] {
            lines.push(format!("{}: {}", label, path.display()));
            if path.exists() {
                let readable =
                    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).is_ok();
                let writable =
                    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).is_ok();
                lines.push(format!(
                    "  Readable: {}",
                    if readable { "yes" } else { "no" }
                ));
                lines.push(format!(
                    "  Writable: {}",
                    if writable { "yes" } else { "no" }
                ));

                // Schema digest
                if readable
                    && let Ok(conn) =
                        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                    && let Ok(sql) = conn.query_row::<String, _, _>(
                        "SELECT sql FROM sqlite_master WHERE name='access' AND type='table'",
                        [],
                        |row| row.get(0),
                    )
                {
                    let mut hasher = sha1_smol::Sha1::new();
                    hasher.update(sql.as_bytes());
                    let hex = hasher.digest().to_string();
                    let short = &hex[..10];
                    let known = if KNOWN_DIGESTS.contains(&short) {
                        "known"
                    } else {
                        "UNKNOWN"
                    };
                    lines.push(format!("  Schema digest: {} ({})", short, known));
                }
            } else {
                lines.push("  Not found".to_string());
            }
            lines.push(String::new());
        }

        lines
    }
}

pub fn nix_is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Truncate a client path to just the binary name
pub fn compact_client(client: &str) -> String {
    if client.starts_with('/') {
        // It's a path — extract just the filename
        std::path::Path::new(client)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| client.to_string())
    } else {
        client.to_string()
    }
}

/// Map auth_value to a display string
pub fn auth_value_display(value: i32) -> String {
    match value {
        0 => "denied".to_string(),
        2 => "granted".to_string(),
        3 => "limited".to_string(),
        v => format!("unknown({})", v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Service name mapping ──────────────────────────────────────────

    #[test]
    fn known_service_keys_resolve_to_human_names() {
        assert_eq!(
            TccDb::service_display_name("kTCCServiceAccessibility"),
            "Accessibility"
        );
        assert_eq!(
            TccDb::service_display_name("kTCCServiceScreenCapture"),
            "Screen Recording"
        );
        assert_eq!(TccDb::service_display_name("kTCCServiceCamera"), "Camera");
        assert_eq!(
            TccDb::service_display_name("kTCCServiceMicrophone"),
            "Microphone"
        );
        assert_eq!(
            TccDb::service_display_name("kTCCServiceSystemPolicyAllFiles"),
            "Full Disk Access"
        );
        assert_eq!(TccDb::service_display_name("kTCCServicePhotos"), "Photos");
    }

    #[test]
    fn unknown_service_key_with_prefix_strips_prefix() {
        // Unknown key with kTCCService prefix should strip the prefix
        assert_eq!(
            TccDb::service_display_name("kTCCServiceSomethingNew"),
            "SomethingNew"
        );
    }

    #[test]
    fn unknown_service_key_without_prefix_returns_raw() {
        // Key without the standard prefix returns as-is
        assert_eq!(
            TccDb::service_display_name("com.example.custom"),
            "com.example.custom"
        );
        assert_eq!(TccDb::service_display_name("FooBar"), "FooBar");
    }

    // ── Auth value display ────────────────────────────────────────────

    #[test]
    fn auth_value_denied() {
        assert_eq!(auth_value_display(0), "denied");
    }

    #[test]
    fn auth_value_granted() {
        assert_eq!(auth_value_display(2), "granted");
    }

    #[test]
    fn auth_value_limited() {
        assert_eq!(auth_value_display(3), "limited");
    }

    #[test]
    fn auth_value_unknown_values() {
        assert_eq!(auth_value_display(1), "unknown(1)");
        assert_eq!(auth_value_display(99), "unknown(99)");
        assert_eq!(auth_value_display(-1), "unknown(-1)");
    }

    // ── DB open authorization hint mapping ───────────────────────────

    #[test]
    fn db_open_auth_denied_on_user_tcc_db_includes_fda_hint() {
        let err = TccError::DbOpen {
            path: PathBuf::from("/Users/test/Library/Application Support/com.apple.TCC/TCC.db"),
            source: "opening database: authorization denied".to_string(),
        };

        let rendered = err.to_string();
        assert!(rendered.contains("Failed to open"));
        assert!(rendered.contains("Full Disk Access"));
        assert!(rendered.contains("Terminal, iTerm, Ghostty, VS Code"));
        assert!(rendered.contains("fully quit and reopen"));
        assert!(rendered.contains("`sudo` does not bypass TCC"));
    }

    #[test]
    fn db_open_auth_denied_on_system_tcc_db_includes_fda_hint() {
        let err = TccError::DbOpen {
            path: PathBuf::from("/Library/Application Support/com.apple.TCC/TCC.db"),
            source: "Open authorization denied".to_string(),
        };

        let rendered = err.to_string();
        assert!(rendered.contains("Full Disk Access"));
    }

    #[test]
    fn db_open_auth_denied_on_non_tcc_path_does_not_include_hint() {
        let err = TccError::DbOpen {
            path: PathBuf::from("/tmp/not-tcc.db"),
            source: "opening database: authorization denied".to_string(),
        };

        let rendered = err.to_string();
        assert!(!rendered.contains("Full Disk Access"));
        assert!(!rendered.contains("`sudo` does not bypass TCC"));
    }

    #[test]
    fn db_open_unable_to_open_on_tcc_path_includes_fda_hint() {
        // Real-world case: SQLite reports "unable to open database file" when
        // FDA blocks the process. The hint must fire here.
        let err = TccError::DbOpen {
            path: PathBuf::from("/Library/Application Support/com.apple.TCC/TCC.db"),
            source:
                "unable to open database file: /Library/Application Support/com.apple.TCC/TCC.db"
                    .to_string(),
        };

        let rendered = err.to_string();
        assert!(rendered.contains("Full Disk Access"));
    }

    #[test]
    fn db_open_unrelated_error_on_tcc_path_does_not_include_hint() {
        let err = TccError::DbOpen {
            path: PathBuf::from("/Library/Application Support/com.apple.TCC/TCC.db"),
            source: "file is not a database".to_string(),
        };

        let rendered = err.to_string();
        assert!(!rendered.contains("Full Disk Access"));
    }

    // ── Compact path display ──────────────────────────────────────────

    #[test]
    fn compact_client_extracts_binary_name_from_path() {
        assert_eq!(compact_client("/usr/local/bin/my-tool"), "my-tool");
        assert_eq!(
            compact_client("/Applications/Safari.app/Contents/MacOS/Safari"),
            "Safari"
        );
    }

    #[test]
    fn compact_client_returns_bundle_id_unchanged() {
        assert_eq!(compact_client("com.apple.Terminal"), "com.apple.Terminal");
        assert_eq!(compact_client("org.mozilla.firefox"), "org.mozilla.firefox");
    }

    #[test]
    fn compact_client_root_path() {
        // Edge case: root path "/"
        assert_eq!(compact_client("/"), "/");
    }

    // ── Client/service filtering (partial match via list) ─────────────

    fn seed_entry(path: &Path, service: &str, client: &str, auth_value: i32) {
        let conn = Connection::open(path).expect("open seed db");
        conn.execute(
            "INSERT INTO access (service, client, client_type, auth_value, auth_reason, auth_version, flags, last_modified)
             VALUES (?1, ?2, 1, ?3, 0, 1, 0, 0)",
            rusqlite::params![service, client, auth_value],
        )
        .expect("seed entry");
    }

    #[test]
    fn client_filter_partial_match() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user.db");
        build_valid_tcc_db(&user);
        seed_entry(&user, "kTCCServiceCamera", "com.apple.Terminal", 2);
        seed_entry(&user, "kTCCServiceMicrophone", "com.google.Chrome", 0);
        seed_entry(&user, "kTCCServiceCamera", "com.apple.Safari", 2);

        let db = TccDb::with_paths(user, dir.path().join("system.db"), DbTarget::User);
        let filtered = db.list(Some("apple"), None).unwrap().entries;
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|e| e.client.contains("apple")));
    }

    #[test]
    fn service_filter_partial_match_display_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user.db");
        build_valid_tcc_db(&user);
        seed_entry(&user, "kTCCServiceCamera", "com.app.a", 2);
        seed_entry(&user, "kTCCServiceMicrophone", "com.app.b", 0);
        seed_entry(&user, "kTCCServiceScreenCapture", "com.app.c", 2);

        let db = TccDb::with_paths(user, dir.path().join("system.db"), DbTarget::User);
        let filtered = db.list(None, Some("Camer")).unwrap().entries;
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].service_raw, "kTCCServiceCamera");
    }

    // ── list error semantics (all-fail vs partial-fail) ───────────────

    #[test]
    fn list_returns_err_when_every_targeted_db_fails_to_read() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bad_user = dir.path().join("user.db");
        let bad_system = dir.path().join("system.db");
        // Files exist but contain no SQLite database, so reads fail rather
        // than returning Ok(empty) (which is what a non-existent path does).
        std::fs::write(&bad_user, b"not a tcc db").expect("write user");
        std::fs::write(&bad_system, b"not a tcc db").expect("write system");

        let db = TccDb::with_paths(bad_user, bad_system, DbTarget::Default);

        let result = db.list(None, None);
        assert!(
            result.is_err(),
            "list must error when every targeted DB fails (regression: would silently return Ok(empty) and JSON consumers couldn't tell apart 'no entries' from 'unreadable DB')"
        );
    }

    /// Build a valid TCC.db with the production schema at `path`.
    fn build_valid_tcc_db(path: &Path) {
        let conn = Connection::open(path).expect("create db");
        conn.execute_batch(
            "CREATE TABLE access (
                service TEXT NOT NULL,
                client TEXT NOT NULL,
                client_type INTEGER NOT NULL,
                auth_value INTEGER NOT NULL DEFAULT 0,
                auth_reason INTEGER NOT NULL DEFAULT 0,
                auth_version INTEGER NOT NULL DEFAULT 1,
                flags INTEGER NOT NULL DEFAULT 0,
                last_modified INTEGER DEFAULT 0,
                PRIMARY KEY (service, client, client_type)
            );",
        )
        .expect("schema");
    }

    #[test]
    fn list_returns_ok_with_partial_results_when_one_db_succeeds() {
        let dir = tempfile::tempdir().expect("tempdir");
        let good_user = dir.path().join("user.db");
        let bad_system = dir.path().join("system.db");
        build_valid_tcc_db(&good_user);
        std::fs::write(&bad_system, b"not a tcc db").expect("write system");

        let db = TccDb::with_paths(good_user, bad_system, DbTarget::Default);

        let result = db.list(None, None);
        assert!(
            result.is_ok(),
            "partial success should still return Ok, got {:?}",
            result.err()
        );
        let listed = result.unwrap();
        assert_eq!(
            listed.entries.len(),
            0,
            "user DB is empty, expected 0 entries"
        );
        assert_eq!(
            listed.warnings.len(),
            1,
            "system DB failure must surface as a structured warning for JSON consumers"
        );
        assert_eq!(listed.warnings[0].kind, ListWarningKind::DbUnreadable);
        assert_eq!(listed.warnings[0].source, "system");
        assert!(
            listed.warnings[0].message.contains("Failed to open")
                || listed.warnings[0].message.contains("Query"),
            "unexpected warning: {}",
            listed.warnings[0].message
        );
    }

    #[test]
    fn list_partial_failure_emits_warning_when_warnings_enabled() {
        // Same scenario as the partial-success test, but with warnings enabled
        // so the per-failure stderr branch is exercised. The visible side
        // effect is a warning on stderr (captured by cargo test); the
        // observable contract is that the function still returns Ok.
        let dir = tempfile::tempdir().expect("tempdir");
        let good_user = dir.path().join("user.db");
        let bad_system = dir.path().join("system.db");
        build_valid_tcc_db(&good_user);
        std::fs::write(&bad_system, b"not a tcc db").expect("write system");

        let db = TccDb::with_paths(good_user, bad_system, DbTarget::Default);
        let result = db.list(None, None);
        assert!(result.is_ok(), "partial success should return Ok");
        let listed = result.unwrap();
        assert_eq!(listed.warnings.len(), 1);
        assert_eq!(listed.warnings[0].kind, ListWarningKind::DbUnreadable);
    }

    #[test]
    fn service_filter_partial_match_raw_key() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user.db");
        build_valid_tcc_db(&user);
        seed_entry(&user, "kTCCServiceCamera", "com.app.a", 2);
        seed_entry(&user, "kTCCServiceMicrophone", "com.app.b", 0);

        let db = TccDb::with_paths(user, dir.path().join("system.db"), DbTarget::User);
        let filtered = db.list(None, Some("kTCCServiceMicro")).unwrap().entries;
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].service_raw, "kTCCServiceMicrophone");
    }

    #[test]
    fn filter_case_insensitive() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user.db");
        build_valid_tcc_db(&user);
        seed_entry(&user, "kTCCServiceCamera", "com.Apple.Terminal", 2);

        let db = TccDb::with_paths(user, dir.path().join("system.db"), DbTarget::User);
        let filtered = db.list(Some("APPLE"), None).unwrap().entries;
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn filter_no_match_returns_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user.db");
        build_valid_tcc_db(&user);
        seed_entry(&user, "kTCCServiceCamera", "com.apple.Terminal", 2);

        let db = TccDb::with_paths(user, dir.path().join("system.db"), DbTarget::User);
        let filtered = db.list(Some("nonexistent"), None).unwrap().entries;
        assert!(filtered.is_empty());
    }

    // ── SERVICE_MAP sanity ────────────────────────────────────────────

    #[test]
    fn service_map_contains_expected_entries() {
        assert!(SERVICE_MAP.contains_key("kTCCServiceAccessibility"));
        assert!(SERVICE_MAP.contains_key("kTCCServiceCamera"));
        assert!(SERVICE_MAP.contains_key("kTCCServiceMicrophone"));
        assert!(SERVICE_MAP.contains_key("kTCCServiceScreenCapture"));
        assert!(SERVICE_MAP.len() > 20);
    }

    // ── Format timestamp ──────────────────────────────────────────────

    #[test]
    fn format_timestamp_zero_returns_na() {
        assert_eq!(TccDb::format_timestamp(0), "N/A");
    }

    #[test]
    fn format_timestamp_large_unix_value() {
        // A recent Unix timestamp should produce a valid date
        let result = TccDb::format_timestamp(1_700_000_000);
        assert!(result.contains("2023"), "Expected 2023 in: {}", result);
    }

    #[test]
    fn format_timestamp_coredata_value() {
        // CoreData timestamp (seconds since 2001-01-01) — small value
        // 700_000_000 + 978_307_200 = 1_678_307_200 → 2023
        let result = TccDb::format_timestamp(700_000_000);
        assert!(
            result.contains("2023") || result.contains("2024"),
            "Got: {}",
            result
        );
    }

    // ── Resolve service name ──────────────────────────────────────────

    fn make_test_db() -> TccDb {
        TccDb::with_paths(
            PathBuf::from("/nonexistent/user.db"),
            PathBuf::from("/nonexistent/system.db"),
            DbTarget::User,
        )
    }

    #[test]
    fn resolve_exact_key() {
        let db = make_test_db();
        assert_eq!(
            db.resolve_service_name("kTCCServiceCamera").unwrap(),
            "kTCCServiceCamera"
        );
    }

    #[test]
    fn resolve_display_name() {
        let db = make_test_db();
        assert_eq!(
            db.resolve_service_name("Camera").unwrap(),
            "kTCCServiceCamera"
        );
    }

    #[test]
    fn resolve_case_insensitive() {
        let db = make_test_db();
        assert_eq!(
            db.resolve_service_name("camera").unwrap(),
            "kTCCServiceCamera"
        );
    }

    #[test]
    fn resolve_ambiguous_errors() {
        let db = make_test_db();
        // "Photo" matches both "Photos" and "Photos (Add Only)"
        let err = db.resolve_service_name("Photo").unwrap_err();
        assert!(
            matches!(err, TccError::AmbiguousService { .. }),
            "Expected AmbiguousService, got: {}",
            err
        );
    }

    #[test]
    fn resolve_unknown_errors() {
        let db = make_test_db();
        let err = db.resolve_service_name("NonexistentService").unwrap_err();
        assert!(matches!(err, TccError::UnknownService(_)));
    }

    #[test]
    fn resolve_short_name_via_prefix() {
        let db = make_test_db();
        assert_eq!(
            db.resolve_service_name("BluetoothAlways").unwrap(),
            "kTCCServiceBluetoothAlways"
        );
    }

    // ── Write operation tests (temp DB) ───────────────────────────────

    fn make_temp_tcc_db() -> (tempfile::TempDir, TccDb) {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = dir.path().join("TCC.db");

        let conn = Connection::open(&db_path).expect("failed to create temp db");
        conn.execute_batch(
            "CREATE TABLE access (
                service TEXT NOT NULL,
                client TEXT NOT NULL,
                client_type INTEGER NOT NULL,
                auth_value INTEGER NOT NULL DEFAULT 0,
                auth_reason INTEGER NOT NULL DEFAULT 0,
                auth_version INTEGER NOT NULL DEFAULT 1,
                flags INTEGER NOT NULL DEFAULT 0,
                last_modified INTEGER DEFAULT 0,
                PRIMARY KEY (service, client, client_type)
            );",
        )
        .expect("failed to create table");
        drop(conn);

        let mut db = TccDb::with_paths(db_path, dir.path().join("system_TCC.db"), DbTarget::User);
        // Fixture DDL is not in KNOWN_DIGESTS; write tests use --force semantics.
        db.set_force(true);

        (dir, db)
    }

    #[test]
    fn grant_inserts_entry() {
        let (_dir, db) = make_temp_tcc_db();
        let result = db.grant("Camera", "com.example.app");
        assert!(result.is_ok(), "grant failed: {:?}", result.err());
        assert!(result.unwrap().message.contains("Granted"));

        let entries = db.list(None, None).unwrap().entries;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].service_raw, "kTCCServiceCamera");
        assert_eq!(entries[0].client, "com.example.app");
        assert_eq!(entries[0].auth_value, 2);
    }

    #[test]
    fn grant_sets_client_type_for_path() {
        let (_dir, db) = make_temp_tcc_db();
        db.grant("Camera", "/usr/bin/test").unwrap();

        let conn = Connection::open(&db.user_db_path).unwrap();
        let client_type: i32 = conn
            .query_row(
                "SELECT client_type FROM access WHERE client = '/usr/bin/test'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(client_type, 0, "Path client should have client_type 0");
    }

    #[test]
    fn grant_sets_client_type_for_bundle_id() {
        let (_dir, db) = make_temp_tcc_db();
        db.grant("Camera", "com.example.app").unwrap();

        let conn = Connection::open(&db.user_db_path).unwrap();
        let client_type: i32 = conn
            .query_row(
                "SELECT client_type FROM access WHERE client = 'com.example.app'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(client_type, 1, "Bundle ID should have client_type 1");
    }

    #[test]
    fn revoke_removes_entry() {
        let (_dir, db) = make_temp_tcc_db();
        db.grant("Camera", "com.example.app").unwrap();

        let result = db.revoke("Camera", "com.example.app");
        assert!(result.is_ok());

        let entries = db.list(None, None).unwrap().entries;
        assert!(entries.is_empty());
    }

    #[test]
    fn revoke_nonexistent_returns_not_found() {
        let (_dir, db) = make_temp_tcc_db();
        let result = db.revoke("Camera", "com.nonexistent.app");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TccError::NotFound { .. }));
    }

    #[test]
    fn enable_sets_auth_value_to_granted() {
        let (_dir, db) = make_temp_tcc_db();
        db.grant("Camera", "com.example.app").unwrap();
        db.disable("Camera", "com.example.app").unwrap();

        let entries = db.list(None, None).unwrap().entries;
        assert_eq!(entries[0].auth_value, 0);

        db.enable("Camera", "com.example.app").unwrap();
        let entries = db.list(None, None).unwrap().entries;
        assert_eq!(entries[0].auth_value, 2);
    }

    #[test]
    fn disable_sets_auth_value_to_denied() {
        let (_dir, db) = make_temp_tcc_db();
        db.grant("Camera", "com.example.app").unwrap();

        db.disable("Camera", "com.example.app").unwrap();
        let entries = db.list(None, None).unwrap().entries;
        assert_eq!(entries[0].auth_value, 0);
    }

    #[test]
    fn enable_nonexistent_returns_not_found() {
        let (_dir, db) = make_temp_tcc_db();
        let result = db.enable("Camera", "com.nonexistent.app");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, TccError::NotFound { .. }));
        if let TccError::NotFound { service, client } = err {
            assert_eq!(service, "Camera");
            assert_eq!(client, "com.nonexistent.app");
        }
        // Remediation lives in Display, not the structured service field.
        let rendered = TccError::NotFound {
            service: "Camera".into(),
            client: "com.nonexistent.app".into(),
        }
        .to_string();
        assert!(rendered.contains("tccutil-rs grant"));
        assert!(!rendered.contains("`tcc grant`"));
    }

    #[test]
    fn operator_messages_use_tccutil_rs_binary_name() {
        let unknown = TccError::UnknownService("Nope".into()).to_string();
        assert!(unknown.contains("tccutil-rs services"));
        assert!(!unknown.contains("`tcc services`"));

        if nix_is_root() {
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user.db");
        build_valid_tcc_db(&user);
        let mut db = TccDb::with_paths(
            user,
            PathBuf::from(TccDb::LIVE_SYSTEM_DB),
            DbTarget::Default,
        );
        db.set_force(true);
        let msg = db
            .grant("Accessibility", "/bin/foo")
            .unwrap_err()
            .to_string();
        assert!(msg.contains("sudo tccutil-rs grant"));
        assert!(!msg.contains("sudo tcc "));
    }

    #[test]
    fn reset_specific_client() {
        let (_dir, db) = make_temp_tcc_db();
        db.grant("Camera", "com.example.a").unwrap();
        db.grant("Camera", "com.example.b").unwrap();

        db.reset("Camera", Some("com.example.a")).unwrap();
        let entries = db.list(None, None).unwrap().entries;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].client, "com.example.b");
    }

    #[test]
    fn reset_all_entries_for_service() {
        let (_dir, db) = make_temp_tcc_db();
        db.grant("Camera", "com.example.a").unwrap();
        db.grant("Camera", "com.example.b").unwrap();
        db.grant("Microphone", "com.example.a").unwrap();

        let result = db.reset("Camera", None).unwrap();
        assert!(result.message.contains("2 deleted"));

        let entries = db.list(None, None).unwrap().entries;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].service_raw, "kTCCServiceMicrophone");
    }

    #[test]
    fn with_paths_constructor() {
        let db = TccDb::with_paths(
            PathBuf::from("/tmp/user.db"),
            PathBuf::from("/tmp/system.db"),
            DbTarget::User,
        );
        assert_eq!(db.user_db_path, PathBuf::from("/tmp/user.db"));
        assert_eq!(db.system_db_path, PathBuf::from("/tmp/system.db"));
    }

    // ── Write routing (user vs system DB) ─────────────────────────────

    fn make_dual_temp_tcc_db(target: DbTarget) -> (tempfile::TempDir, TccDb) {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user.db");
        let system = dir.path().join("system.db");
        build_valid_tcc_db(&user);
        build_valid_tcc_db(&system);
        let mut db = TccDb::with_paths(user, system, target);
        db.set_force(true);
        (dir, db)
    }

    fn count_rows(path: &Path) -> i64 {
        let conn = Connection::open(path).expect("open count");
        conn.query_row("SELECT COUNT(*) FROM access", [], |row| row.get(0))
            .expect("count")
    }

    #[test]
    fn default_grant_camera_writes_user_db() {
        let (_dir, db) = make_dual_temp_tcc_db(DbTarget::Default);
        db.grant("Camera", "com.example.app").unwrap();
        assert_eq!(count_rows(&db.user_db_path), 1);
        assert_eq!(count_rows(&db.system_db_path), 0);
    }

    #[test]
    fn default_grant_desktop_folder_writes_user_db() {
        // Folder SystemPolicy* services live in the per-user TCC.db.
        let (_dir, db) = make_dual_temp_tcc_db(DbTarget::Default);
        db.grant("Desktop Folder", "com.example.app").unwrap();
        assert_eq!(count_rows(&db.user_db_path), 1);
        assert_eq!(count_rows(&db.system_db_path), 0);
    }

    #[test]
    fn default_grant_accessibility_writes_system_temp_db() {
        let (_dir, db) = make_dual_temp_tcc_db(DbTarget::Default);
        db.grant("Accessibility", "com.example.app").unwrap();
        assert_eq!(count_rows(&db.user_db_path), 0);
        assert_eq!(count_rows(&db.system_db_path), 1);
    }

    #[test]
    fn default_grant_accessibility_requires_root_for_live_system_db() {
        if nix_is_root() {
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user.db");
        build_valid_tcc_db(&user);
        let mut db = TccDb::with_paths(
            user,
            PathBuf::from(TccDb::LIVE_SYSTEM_DB),
            DbTarget::Default,
        );
        db.set_force(true);
        let err = db.grant("Accessibility", "com.example.app").unwrap_err();
        assert!(
            matches!(err, TccError::NeedsRoot { .. }),
            "expected NeedsRoot, got: {}",
            err
        );
    }

    #[test]
    fn default_grant_full_disk_access_requires_root_for_live_system_db() {
        if nix_is_root() {
            return;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user.db");
        build_valid_tcc_db(&user);
        let mut db = TccDb::with_paths(
            user,
            PathBuf::from(TccDb::LIVE_SYSTEM_DB),
            DbTarget::Default,
        );
        db.set_force(true);
        let err = db.grant("Full Disk Access", "com.example.app").unwrap_err();
        assert!(
            matches!(err, TccError::NeedsRoot { .. }),
            "FDA must route to live system DB, got: {}",
            err
        );
    }

    #[test]
    fn user_target_grant_accessibility_writes_user_db_without_root() {
        let (_dir, db) = make_dual_temp_tcc_db(DbTarget::User);
        db.grant("Accessibility", "com.example.app").unwrap();
        assert_eq!(count_rows(&db.user_db_path), 1);
        assert_eq!(count_rows(&db.system_db_path), 0);
    }

    #[test]
    fn default_reset_user_only_service_succeeds_without_root() {
        if nix_is_root() {
            return;
        }
        let (_dir, db) = make_dual_temp_tcc_db(DbTarget::Default);
        // Seed via User target so we don't need root for Camera.
        let user_db = {
            let mut db = TccDb::with_paths(
                db.user_db_path.clone(),
                db.system_db_path.clone(),
                DbTarget::User,
            );
            db.set_force(true);
            db
        };
        user_db.grant("Camera", "com.example.a").unwrap();
        user_db.grant("Camera", "com.example.b").unwrap();
        assert_eq!(count_rows(&db.user_db_path), 2);

        let result = db.reset("Camera", None).unwrap();
        assert!(result.message.contains("2 deleted"));
        assert_eq!(count_rows(&db.user_db_path), 0);
        assert_eq!(count_rows(&db.system_db_path), 0);
    }

    #[test]
    fn default_reset_deletes_both_temp_dbs_without_live_system() {
        // Temp system paths do not require root; Default reset must clear both.
        // (Do not probe the live system TCC.db — CI runners may have a readable
        // empty system DB, which would make a live-path preflight succeed and
        // only delete the user fixture.)
        let (_dir, db) = make_dual_temp_tcc_db(DbTarget::Default);
        seed_entry(&db.user_db_path, "kTCCServiceCamera", "com.example.user", 2);
        seed_entry(
            &db.system_db_path,
            "kTCCServiceCamera",
            "com.example.system",
            2,
        );
        let result = db.reset("Camera", None).unwrap();
        assert!(result.message.contains("2 deleted"));
        assert_eq!(count_rows(&db.user_db_path), 0);
        assert_eq!(count_rows(&db.system_db_path), 0);
    }

    #[test]
    fn default_reset_atomic_rolls_back_when_second_delete_aborts() {
        let (_dir, db) = make_dual_temp_tcc_db(DbTarget::Default);
        seed_entry(&db.user_db_path, "kTCCServiceCamera", "com.example.user", 2);
        seed_entry(
            &db.system_db_path,
            "kTCCServiceCamera",
            "com.example.system",
            2,
        );
        // Abort any DELETE on the system DB so the ATTACH transaction must roll back.
        let conn = Connection::open(&db.system_db_path).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER deny_delete BEFORE DELETE ON access BEGIN
                SELECT RAISE(ABORT, 'injected delete failure');
             END;",
        )
        .unwrap();
        drop(conn);

        let err = db.reset("Camera", None).unwrap_err();
        assert!(
            matches!(err, TccError::WriteFailed(_)),
            "expected WriteFailed from aborted txn, got: {}",
            err
        );
        assert_eq!(
            count_rows(&db.user_db_path),
            1,
            "user rows must remain after rolled-back dual reset (statement abort)"
        );
        assert_eq!(count_rows(&db.system_db_path), 1);
    }

    #[test]
    fn grant_allows_unknown_schema_with_force() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user.db");
        build_valid_tcc_db(&user);
        let mut db = TccDb::with_paths(user, dir.path().join("system.db"), DbTarget::User);
        db.set_force(true);
        let result = db.grant("Camera", "com.example.app").unwrap();
        assert_eq!(count_rows(&db.user_db_path), 1);
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.warnings[0].kind, WriteWarningKind::UnknownSchema);
    }

    #[test]
    fn grant_rejects_unknown_schema_without_force() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user.db");
        build_valid_tcc_db(&user);
        let db = TccDb::with_paths(user, dir.path().join("system.db"), DbTarget::User);
        let err = db.grant("Camera", "com.example.app").unwrap_err();
        assert!(
            matches!(err, TccError::SchemaInvalid(_)),
            "expected SchemaInvalid, got: {}",
            err
        );
        assert!(err.to_string().contains("--force"));
        assert_eq!(count_rows(&db.user_db_path), 0);
    }

    #[test]
    fn grant_rejects_db_missing_access_table() {
        let dir = tempfile::tempdir().expect("tempdir");
        let user = dir.path().join("user.db");
        Connection::open(&user)
            .expect("open")
            .execute_batch("CREATE TABLE unrelated (id INTEGER);")
            .expect("seed");
        let db = TccDb::with_paths(user, dir.path().join("system.db"), DbTarget::User);
        let err = db.grant("Camera", "com.example.app").unwrap_err();
        assert!(
            matches!(err, TccError::SchemaInvalid(_)),
            "expected SchemaInvalid, got: {}",
            err
        );
        assert!(err.to_string().contains("access table"));
    }
}
