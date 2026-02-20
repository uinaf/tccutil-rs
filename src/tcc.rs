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
                write!(f, "Failed to open {}: {}", path.display(), source)
            }
            TccError::NotFound { service, client } => {
                write!(
                    f,
                    "No entry found for service '{}' and client '{}'",
                    service, client
                )
            }
            TccError::NeedsRoot { message } => write!(f, "{}", message),
            TccError::UnknownService(s) => write!(
                f,
                "Unknown service '{}'. Run `tcc services` to see available services.",
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

#[derive(Debug)]
pub struct TccEntry {
    pub service_raw: String,
    pub service_display: String,
    pub client: String,
    pub auth_value: i32,
    pub last_modified: String,
    pub is_system: bool,
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
    suppress_warnings: bool,
}

impl TccDb {
    pub fn new(target: DbTarget) -> Result<Self, TccError> {
        let home = dirs::home_dir().ok_or(TccError::HomeDirNotFound)?;
        Ok(Self {
            user_db_path: home.join("Library/Application Support/com.apple.TCC/TCC.db"),
            system_db_path: PathBuf::from("/Library/Application Support/com.apple.TCC/TCC.db"),
            target,
            suppress_warnings: false,
        })
    }

    #[cfg(test)]
    pub fn with_paths(user: PathBuf, system: PathBuf, target: DbTarget) -> Self {
        Self {
            user_db_path: user,
            system_db_path: system,
            target,
            suppress_warnings: false,
        }
    }

    pub fn set_suppress_warnings(&mut self, suppress_warnings: bool) {
        self.suppress_warnings = suppress_warnings;
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
        is_system: bool,
        emit_warnings: bool,
    ) -> Result<Vec<TccEntry>, TccError> {
        if !path.exists() {
            return Ok(vec![]);
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
        for result in rows {
            match result {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    if emit_warnings {
                        eprintln!(
                            "Warning: skipping malformed row in {}: {}",
                            path.display(),
                            e
                        );
                    }
                }
            }
        }

        Ok(entries)
    }

    pub fn list(
        &self,
        client_filter: Option<&str>,
        service_filter: Option<&str>,
    ) -> Result<Vec<TccEntry>, TccError> {
        let mut entries = Vec::new();

        if self.target == DbTarget::Default || self.target == DbTarget::User {
            match Self::read_db(&self.user_db_path, false, !self.suppress_warnings) {
                Ok(mut e) => entries.append(&mut e),
                Err(e) => {
                    if !self.suppress_warnings {
                        eprintln!("Warning: {}", e);
                    }
                }
            }
        }

        if self.target == DbTarget::Default {
            match Self::read_db(&self.system_db_path, true, !self.suppress_warnings) {
                Ok(mut e) => entries.append(&mut e),
                Err(e) => {
                    if !self.suppress_warnings {
                        eprintln!("Warning: {}", e);
                    }
                }
            }
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

        Ok(entries)
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

    fn is_system_service(service: &str) -> bool {
        matches!(
            service,
            "kTCCServiceAccessibility"
                | "kTCCServiceScreenCapture"
                | "kTCCServiceListenEvent"
                | "kTCCServicePostEvent"
                | "kTCCServiceEndpointSecurityClient"
                | "kTCCServiceDeveloperTool"
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

    /// Check if root is needed and we don't have it
    fn check_root_for_write(
        &self,
        service_key: &str,
        action: &str,
        service_input: &str,
        client: &str,
    ) -> Result<(), TccError> {
        let db_path = self.write_db_path(service_key);
        if db_path == self.system_db_path && !nix_is_root() {
            return Err(TccError::NeedsRoot {
                message: format!(
                    "Service '{}' requires the system TCC database.\n\
                     Run with sudo: sudo tcc {} {} {}",
                    Self::service_display_name(service_key),
                    action,
                    service_input,
                    client
                ),
            });
        }
        Ok(())
    }

    /// Validate the DB schema before writing. Returns Ok with an optional warning.
    fn validate_schema(conn: &Connection) -> Result<Option<String>, TccError> {
        let digest: Option<String> = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name='access' AND type='table'",
                [],
                |row| row.get(0),
            )
            .ok();

        if let Some(sql) = digest {
            let mut hasher = sha1_smol::Sha1::new();
            hasher.update(sql.as_bytes());
            let hex = hasher.digest().to_string();
            let short = &hex[..10];

            if KNOWN_DIGESTS.contains(&short) {
                Ok(None)
            } else {
                Ok(Some(format!(
                    "Warning: Unknown TCC database schema (digest: {}). Proceeding anyway — results may vary.",
                    short
                )))
            }
        } else {
            Err(TccError::SchemaInvalid(
                "Could not read TCC database schema. The access table may not exist.".to_string(),
            ))
        }
    }

    /// Open a writable connection with schema validation
    fn open_writable(&self, service_key: &str) -> Result<(Connection, Option<String>), TccError> {
        let db_path = self.write_db_path(service_key);
        let conn = Connection::open(db_path).map_err(|e| TccError::DbOpen {
            path: db_path.to_path_buf(),
            source: e.to_string(),
        })?;
        let warning = Self::validate_schema(&conn)?;
        Ok((conn, warning))
    }

    pub fn grant(&self, service: &str, client: &str) -> Result<String, TccError> {
        let service_key = self.resolve_service_name(service)?;
        self.check_root_for_write(&service_key, "grant", service, client)?;

        let (conn, warning) = self.open_writable(&service_key)?;
        if let Some(w) = &warning
            && !self.suppress_warnings
        {
            eprintln!("{}", w);
        }

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

        Ok(format!(
            "Granted {} access for '{}'",
            Self::service_display_name(&service_key),
            client
        ))
    }

    pub fn revoke(&self, service: &str, client: &str) -> Result<String, TccError> {
        let service_key = self.resolve_service_name(service)?;
        self.check_root_for_write(&service_key, "revoke", service, client)?;

        let (conn, warning) = self.open_writable(&service_key)?;
        if let Some(w) = &warning
            && !self.suppress_warnings
        {
            eprintln!("{}", w);
        }

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
            Ok(format!(
                "Revoked {} access for '{}'",
                Self::service_display_name(&service_key),
                client
            ))
        }
    }

    pub fn enable(&self, service: &str, client: &str) -> Result<String, TccError> {
        let service_key = self.resolve_service_name(service)?;
        self.check_root_for_write(&service_key, "enable", service, client)?;

        let (conn, warning) = self.open_writable(&service_key)?;
        if let Some(w) = &warning
            && !self.suppress_warnings
        {
            eprintln!("{}", w);
        }

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
                service: format!(
                    "{}. Use `tcc grant` to insert a new entry",
                    Self::service_display_name(&service_key)
                ),
                client: client.to_string(),
            })
        } else {
            Ok(format!(
                "Enabled {} access for '{}'",
                Self::service_display_name(&service_key),
                client
            ))
        }
    }

    pub fn disable(&self, service: &str, client: &str) -> Result<String, TccError> {
        let service_key = self.resolve_service_name(service)?;
        self.check_root_for_write(&service_key, "disable", service, client)?;

        let (conn, warning) = self.open_writable(&service_key)?;
        if let Some(w) = &warning
            && !self.suppress_warnings
        {
            eprintln!("{}", w);
        }

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
            Ok(format!(
                "Disabled {} access for '{}'",
                Self::service_display_name(&service_key),
                client
            ))
        }
    }

    pub fn reset(&self, service: &str, client: Option<&str>) -> Result<String, TccError> {
        let service_key = self.resolve_service_name(service)?;

        if let Some(c) = client {
            // Delete specific client entry
            self.check_root_for_write(&service_key, "reset", service, c)?;

            let (conn, warning) = self.open_writable(&service_key)?;
            if let Some(w) = &warning
                && !self.suppress_warnings
            {
                eprintln!("{}", w);
            }

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
                Ok(format!(
                    "Reset {} entry for '{}'",
                    Self::service_display_name(&service_key),
                    c
                ))
            }
        } else {
            // Delete all entries for this service
            // For default target, try to reset in both DBs
            let mut total_deleted = 0usize;
            let mut errors = Vec::new();

            let paths: Vec<(&Path, &str)> = match self.target {
                DbTarget::User => vec![(&self.user_db_path, "user")],
                DbTarget::Default => vec![
                    (&self.user_db_path, "user"),
                    (&self.system_db_path, "system"),
                ],
            };

            for (db_path, label) in paths {
                if !db_path.exists() {
                    continue;
                }
                // Check root for system DB writes
                if db_path == self.system_db_path && !nix_is_root() {
                    return Err(TccError::NeedsRoot {
                        message: format!(
                            "Resetting all '{}' entries requires the system TCC database.\n\
                             Run with sudo: sudo tcc reset {}",
                            Self::service_display_name(&service_key),
                            service
                        ),
                    });
                }
                match Connection::open(db_path) {
                    Ok(conn) => {
                        if let Err(e) = Self::validate_schema(&conn) {
                            errors.push(format!("{} DB: {}", label, e));
                            continue;
                        }
                        match conn.execute(
                            "DELETE FROM access WHERE service = ?1",
                            rusqlite::params![service_key],
                        ) {
                            Ok(n) => total_deleted += n,
                            Err(e) => errors.push(format!("{} DB: {}", label, e)),
                        }
                    }
                    Err(e) => errors.push(format!("{} DB: {}", label, e)),
                }
            }

            if total_deleted == 0 && !errors.is_empty() {
                Err(TccError::WriteFailed(format!(
                    "Failed to reset: {}",
                    errors.join("; ")
                )))
            } else {
                let mut msg = format!(
                    "Reset all {} entries ({} deleted)",
                    Self::service_display_name(&service_key),
                    total_deleted
                );
                for e in errors {
                    msg.push_str(&format!("\nWarning: {}", e));
                }
                Ok(msg)
            }
        }
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

    // ── Client/service filtering (partial match) ──────────────────────

    #[test]
    fn client_filter_partial_match() {
        let entries = vec![
            make_entry("kTCCServiceCamera", "com.apple.Terminal", 2),
            make_entry("kTCCServiceMicrophone", "com.google.Chrome", 0),
            make_entry("kTCCServiceCamera", "com.apple.Safari", 2),
        ];

        let filtered = filter_entries(entries, Some("apple"), None);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|e| e.client.contains("apple")));
    }

    #[test]
    fn service_filter_partial_match_display_name() {
        let entries = vec![
            make_entry("kTCCServiceCamera", "com.app.a", 2),
            make_entry("kTCCServiceMicrophone", "com.app.b", 0),
            make_entry("kTCCServiceScreenCapture", "com.app.c", 2),
        ];

        // Matches "Camera" display name
        let filtered = filter_entries(entries, None, Some("Camer"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].service_raw, "kTCCServiceCamera");
    }

    #[test]
    fn service_filter_partial_match_raw_key() {
        let entries = vec![
            make_entry("kTCCServiceCamera", "com.app.a", 2),
            make_entry("kTCCServiceMicrophone", "com.app.b", 0),
        ];

        // Matches raw key
        let filtered = filter_entries(entries, None, Some("kTCCServiceMicro"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].service_raw, "kTCCServiceMicrophone");
    }

    #[test]
    fn filter_case_insensitive() {
        let entries = vec![make_entry("kTCCServiceCamera", "com.Apple.Terminal", 2)];

        let filtered = filter_entries(entries, Some("APPLE"), None);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn filter_no_match_returns_empty() {
        let entries = vec![make_entry("kTCCServiceCamera", "com.apple.Terminal", 2)];

        let filtered = filter_entries(entries, Some("nonexistent"), None);
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

    // ── Helpers ───────────────────────────────────────────────────────

    fn make_entry(service_raw: &str, client: &str, auth_value: i32) -> TccEntry {
        TccEntry {
            service_raw: service_raw.to_string(),
            service_display: TccDb::service_display_name(service_raw),
            client: client.to_string(),
            auth_value,
            last_modified: "2024-01-01 00:00:00".to_string(),
            is_system: false,
        }
    }

    /// Applies the same filtering logic as TccDb::list
    fn filter_entries(
        mut entries: Vec<TccEntry>,
        client_filter: Option<&str>,
        service_filter: Option<&str>,
    ) -> Vec<TccEntry> {
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
        entries
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

        let db = TccDb::with_paths(db_path, dir.path().join("system_TCC.db"), DbTarget::User);

        (dir, db)
    }

    #[test]
    fn grant_inserts_entry() {
        let (_dir, db) = make_temp_tcc_db();
        let result = db.grant("Camera", "com.example.app");
        assert!(result.is_ok(), "grant failed: {:?}", result.err());
        assert!(result.unwrap().contains("Granted"));

        let entries = db.list(None, None).unwrap();
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

        let entries = db.list(None, None).unwrap();
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

        let entries = db.list(None, None).unwrap();
        assert_eq!(entries[0].auth_value, 0);

        db.enable("Camera", "com.example.app").unwrap();
        let entries = db.list(None, None).unwrap();
        assert_eq!(entries[0].auth_value, 2);
    }

    #[test]
    fn disable_sets_auth_value_to_denied() {
        let (_dir, db) = make_temp_tcc_db();
        db.grant("Camera", "com.example.app").unwrap();

        db.disable("Camera", "com.example.app").unwrap();
        let entries = db.list(None, None).unwrap();
        assert_eq!(entries[0].auth_value, 0);
    }

    #[test]
    fn enable_nonexistent_returns_not_found() {
        let (_dir, db) = make_temp_tcc_db();
        let result = db.enable("Camera", "com.nonexistent.app");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TccError::NotFound { .. }));
    }

    #[test]
    fn reset_specific_client() {
        let (_dir, db) = make_temp_tcc_db();
        db.grant("Camera", "com.example.a").unwrap();
        db.grant("Camera", "com.example.b").unwrap();

        db.reset("Camera", Some("com.example.a")).unwrap();
        let entries = db.list(None, None).unwrap();
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
        assert!(result.contains("2 deleted"));

        let entries = db.list(None, None).unwrap();
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
}
