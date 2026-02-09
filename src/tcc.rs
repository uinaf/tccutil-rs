use chrono::{Local, TimeZone};
use rusqlite::{Connection, OpenFlags};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

pub static SERVICE_MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("kTCCServiceAccessibility", "Accessibility");
    m.insert("kTCCServiceScreenCapture", "Screen Recording");
    m.insert("kTCCServiceSystemPolicyAllFiles", "Full Disk Access");
    m.insert("kTCCServiceSystemPolicySysAdminFiles", "Administer Computer (SysAdmin)");
    m.insert("kTCCServiceSystemPolicyDesktopFolder", "Desktop Folder");
    m.insert("kTCCServiceSystemPolicyDocumentsFolder", "Documents Folder");
    m.insert("kTCCServiceSystemPolicyDownloadsFolder", "Downloads Folder");
    m.insert("kTCCServiceSystemPolicyNetworkVolumes", "Network Volumes");
    m.insert("kTCCServiceSystemPolicyRemovableVolumes", "Removable Volumes");
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
    m
});

pub struct TccEntry {
    pub service_raw: String,
    pub service_display: String,
    pub client: String,
    pub auth_value: i32,
    pub last_modified: String,
    pub is_system: bool,
}

pub struct TccDb {
    user_db_path: PathBuf,
    system_db_path: PathBuf,
}

impl TccDb {
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("Cannot determine home directory");
        Self {
            user_db_path: home
                .join("Library/Application Support/com.apple.TCC/TCC.db"),
            system_db_path: PathBuf::from(
                "/Library/Application Support/com.apple.TCC/TCC.db",
            ),
        }
    }

    fn format_timestamp(ts: i64) -> String {
        if ts == 0 {
            return "N/A".to_string();
        }
        // macOS TCC uses CoreData timestamps (seconds since 2001-01-01) or Unix timestamps.
        // Try to detect: if value is small (< ~1e9), it's likely CoreData epoch
        let unix_ts = if ts < 1_000_000_000 {
            // CoreData epoch: 2001-01-01 00:00:00 UTC = 978307200 Unix
            ts + 978_307_200
        } else {
            ts
        };

        match Local.timestamp_opt(unix_ts, 0) {
            chrono::LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            _ => format!("{}", ts),
        }
    }

    fn service_display_name(raw: &str) -> String {
        SERVICE_MAP
            .get(raw)
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                // Strip kTCCService prefix if present
                raw.strip_prefix("kTCCService")
                    .unwrap_or(raw)
                    .to_string()
            })
    }

    fn read_db(
        path: &PathBuf,
        is_system: bool,
    ) -> Result<Vec<TccEntry>, String> {
        if !path.exists() {
            return Ok(vec![]);
        }

        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;

        // TCC.db schema varies across macOS versions. Try common column names.
        let query = "SELECT service, client, auth_value, \
                     COALESCE(last_modified, auth_reason, 0) as modified \
                     FROM access";

        let result = conn.prepare(query);
        let mut stmt = match result {
            Ok(s) => s,
            Err(_) => {
                // Fallback: try without last_modified
                let fallback = "SELECT service, client, auth_value, 0 as modified FROM access";
                conn.prepare(fallback)
                    .map_err(|e| format!("Query failed on {}: {}", path.display(), e))?
            }
        };

        let entries = stmt
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
            .map_err(|e| format!("Query error on {}: {}", path.display(), e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(entries)
    }

    pub fn list(
        &self,
        client_filter: Option<&str>,
        service_filter: Option<&str>,
    ) -> Result<Vec<TccEntry>, String> {
        let mut entries = Vec::new();

        // Read user DB
        match Self::read_db(&self.user_db_path, false) {
            Ok(mut e) => entries.append(&mut e),
            Err(e) => eprintln!("Warning: {}", e),
        }

        // Read system DB (may fail without FDA)
        match Self::read_db(&self.system_db_path, true) {
            Ok(mut e) => entries.append(&mut e),
            Err(e) => eprintln!("Warning: {}", e),
        }

        // Apply filters
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

        // Sort by service then client
        entries.sort_by(|a, b| {
            a.service_display
                .cmp(&b.service_display)
                .then(a.client.cmp(&b.client))
        });

        Ok(entries)
    }

    fn resolve_service_name(&self, input: &str) -> Result<String, String> {
        // Check if it's already an internal name
        if SERVICE_MAP.contains_key(input) {
            return Ok(input.to_string());
        }
        // Try to find by display name (case-insensitive)
        let input_lower = input.to_lowercase();
        for (key, display) in SERVICE_MAP.iter() {
            if display.to_lowercase() == input_lower {
                return Ok(key.to_string());
            }
        }
        // Try partial match
        for (key, display) in SERVICE_MAP.iter() {
            if display.to_lowercase().contains(&input_lower) {
                return Ok(key.to_string());
            }
        }
        // Try with kTCCService prefix
        let prefixed = format!("kTCCService{}", input);
        if SERVICE_MAP.contains_key(prefixed.as_str()) {
            return Ok(prefixed);
        }
        Err(format!(
            "Unknown service '{}'. Run `tcc services` to see available services.",
            input
        ))
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

    pub fn grant(&self, service: &str, client: &str) -> Result<String, String> {
        let service_key = self.resolve_service_name(service)?;
        let is_system = Self::is_system_service(&service_key);

        let db_path = if is_system {
            &self.system_db_path
        } else {
            &self.user_db_path
        };

        if is_system {
            // Check if running as root
            if !nix_is_root() {
                return Err(format!(
                    "Service '{}' requires the system TCC database.\n\
                     Run with sudo: sudo tcc grant {} {}",
                    Self::service_display_name(&service_key),
                    service,
                    client
                ));
            }
        }

        let conn = Connection::open(db_path)
            .map_err(|e| format!("Failed to open {}: {}", db_path.display(), e))?;

        // Try INSERT OR REPLACE
        let now = chrono::Utc::now().timestamp() - 978_307_200; // CoreData epoch
        let sql = "INSERT OR REPLACE INTO access \
                   (service, client, client_type, auth_value, auth_reason, auth_version, flags, last_modified) \
                   VALUES (?1, ?2, 0, 2, 0, 1, 0, ?3)";

        conn.execute(sql, rusqlite::params![service_key, client, now])
            .map_err(|e| format!("Failed to grant: {}. Note: SIP may prevent TCC.db writes on macOS 10.14+", e))?;

        Ok(format!(
            "Granted {} access for '{}'",
            Self::service_display_name(&service_key),
            client
        ))
    }

    pub fn revoke(&self, service: &str, client: &str) -> Result<String, String> {
        let service_key = self.resolve_service_name(service)?;
        let is_system = Self::is_system_service(&service_key);

        let db_path = if is_system {
            &self.system_db_path
        } else {
            &self.user_db_path
        };

        if is_system && !nix_is_root() {
            return Err(format!(
                "Service '{}' requires the system TCC database.\n\
                 Run with sudo: sudo tcc revoke {} {}",
                Self::service_display_name(&service_key),
                service,
                client
            ));
        }

        let conn = Connection::open(db_path)
            .map_err(|e| format!("Failed to open {}: {}", db_path.display(), e))?;

        let deleted = conn
            .execute(
                "DELETE FROM access WHERE service = ?1 AND client = ?2",
                rusqlite::params![service_key, client],
            )
            .map_err(|e| format!("Failed to revoke: {}. Note: SIP may prevent TCC.db writes.", e))?;

        if deleted == 0 {
            Err(format!(
                "No entry found for service '{}' and client '{}'",
                Self::service_display_name(&service_key),
                client
            ))
        } else {
            Ok(format!(
                "Revoked {} access for '{}'",
                Self::service_display_name(&service_key),
                client
            ))
        }
    }
}

fn nix_is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}
