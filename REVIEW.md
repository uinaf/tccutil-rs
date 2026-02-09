# Code Review: tcc-cli

Reviewed: 2026-02-09
Files: `src/main.rs`, `src/tcc.rs`, `tests/integration.rs`, `Cargo.toml`

---

## 1. BUGS & LOGIC ISSUES

### 1.1 Silent row-level errors in `read_db` — `.filter_map(|r| r.ok())`

**File:** `src/tcc.rs:159`
**Severity:** HIGH

```rust
.filter_map(|r| r.ok())
```

If individual rows fail to deserialize (e.g. a `NULL` in `service` or `client`, or a type mismatch), those rows are silently dropped. The caller has no idea entries were lost. In a security-sensitive tool that manages permissions, silently omitting entries is dangerous — a user could believe a permission doesn't exist when it does.

**Fix:** Collect errors and either return them or emit warnings:
```rust
let mut entries = Vec::new();
for result in rows {
    match result {
        Ok(entry) => entries.push(entry),
        Err(e) => eprintln!("Warning: skipping malformed row in {}: {}", path.display(), e),
    }
}
```

---

### 1.2 `resolve_service_name` partial match is ambiguous

**File:** `src/tcc.rs:217-220`
**Severity:** MEDIUM

The second loop does a `contains` match, returning the **first** HashMap iteration hit. HashMap iteration order is non-deterministic, so if a user types `"Files"` it could match `Full Disk Access`, `Desktop Folder`, `Documents Folder`, `Downloads Folder`, `Developer Files`, `Removable Volumes`, or `Network Volumes` — and which one you get is random.

**Fix:** Either require exact matches (with a helpful error listing close matches), or collect all partial matches and error if there's more than one:
```rust
let partial_matches: Vec<_> = SERVICE_MAP.iter()
    .filter(|(_, display)| display.to_lowercase().contains(&input_lower))
    .collect();
match partial_matches.len() {
    0 => { /* fall through to kTCCService prefix check */ }
    1 => return Ok(partial_matches[0].0.to_string()),
    _ => return Err(format!(
        "Ambiguous service '{}'. Matches: {}",
        input,
        partial_matches.iter().map(|(_, d)| *d).collect::<Vec<_>>().join(", ")
    )),
}
```

---

### 1.3 `grant` hardcodes `client_type = 0` — incorrect for bundle IDs

**File:** `src/tcc.rs:323`
**Severity:** MEDIUM

```rust
VALUES (?1, ?2, 0, 2, 0, 1, 0, ?3)
```

TCC uses `client_type = 0` for absolute paths and `client_type = 1` for bundle identifiers. Hardcoding `0` means grants for bundle IDs (e.g. `com.apple.Terminal`) will have the wrong client type, which may cause macOS to not recognize the entry.

**Fix:** Detect the client type:
```rust
let client_type: i32 = if client.starts_with('/') { 0 } else { 1 };
```
Then bind it as a parameter.

---

### 1.4 `reset` without client skips schema validation and root check

**File:** `src/tcc.rs:461-506`
**Severity:** MEDIUM

The `reset` all-entries branch (no client specified) opens DB connections directly with `Connection::open()` instead of going through `open_writable()`, bypassing:
1. Schema validation (`validate_schema`)
2. The root check for system DB writes

A non-root user running `tcc reset Accessibility` (no client) will attempt to open the system DB for writing and fail with a confusing SQLite error instead of the friendly "Run with sudo" message.

**Fix:** Use the same `check_root_for_write` / `open_writable` path, or at minimum check `nix_is_root()` before attempting writes to the system DB path.

---

### 1.5 `format_timestamp` threshold is fragile

**File:** `src/tcc.rs:97`
**Severity:** LOW

```rust
let unix_ts = if ts < 1_000_000_000 {
    ts + 978_307_200
} else {
    ts
};
```

The cutoff `1_000_000_000` corresponds to Unix epoch 2001-09-09. Any CoreData timestamp after `~2002-09-09` (i.e. `1_000_000_000 - 978_307_200 = 21_692_800` seconds from 2001-01-01, which is mid-2001) would be treated as a Unix timestamp. In practice TCC entries from modern macOS will have CoreData timestamps in the hundreds of millions so this works, but the logic is not self-documenting and could silently misinterpret timestamps.

**Fix:** Add a comment clarifying the heuristic, or use a more robust check (e.g., reasonable date range validation).

---

### 1.6 `enable`/`disable` don't update `last_modified`

**File:** `src/tcc.rs:378, 409`
**Severity:** LOW

`grant` sets `last_modified` to the current time, but `enable` and `disable` only flip `auth_value` without updating `last_modified`. This means the displayed timestamp will be stale after toggling a permission.

**Fix:**
```sql
UPDATE access SET auth_value = ?3, last_modified = ?4 WHERE service = ?1 AND client = ?2
```

---

## 2. SECURITY

### 2.1 No symlink protection on DB paths

**File:** `src/tcc.rs:83-90, 120-126`
**Severity:** MEDIUM

The tool trusts `~/.../TCC.db` and `/Library/.../TCC.db` as-is. If an attacker can place a symlink at the user DB path pointing to another SQLite database, this tool will happily read/write it. While macOS sandbox protections limit this in practice, and the tool already requires the user to run it explicitly, there is no `O_NOFOLLOW`-style protection.

**Fix:** Consider checking `fs::symlink_metadata()` and refusing to open symlinked DB paths, or at minimum document this as a known limitation.

---

### 2.2 `info()` shells out to `sw_vers` and `csrutil` — not a vuln, but worth noting

**File:** `src/tcc.rs:517-529`
**Severity:** LOW

`Command::new("sw_vers")` and `Command::new("csrutil")` rely on `$PATH` resolution. If someone has a malicious `sw_vers` or `csrutil` earlier in PATH, it would be executed. In practice this tool runs as the current user (or root), so this is not a new attack surface beyond what already exists, but using absolute paths (`/usr/bin/sw_vers`, `/usr/bin/csrutil`) would be slightly more defensive.

---

## 3. CODE QUALITY

### 3.1 `Result<String, String>` error type loses structure

**File:** `src/tcc.rs` (all public methods)
**Severity:** MEDIUM

Every method returns `Result<String, String>`. This is fine for a CLI but means:
- Callers can't programmatically distinguish error kinds (DB not found vs. SIP blocked vs. no entry found)
- The `reset` method returns `Ok(msg)` with embedded warnings that should be separate

For a CLI this is acceptable but limits testability and reuse. A proper error enum would be better:
```rust
enum TccError {
    DbOpen { path: PathBuf, source: rusqlite::Error },
    NotFound { service: String, client: String },
    NeedsRoot { service: String },
    UnknownService(String),
    SipBlocked(String),
}
```

---

### 3.2 `expect()` calls in non-test code

**File:** `src/tcc.rs:84, 510`
**Severity:** MEDIUM

```rust
let home = dirs::home_dir().expect("Cannot determine home directory");
```

This appears in both `TccDb::new()` and `TccDb::info()`. If `HOME` is unset (e.g., in a cron job or stripped environment), this panics. Since the convention is "no panics in library code," these should return `Result`.

**Fix:** Make `TccDb::new()` return `Result<Self, String>` and propagate the error.

---

### 3.3 `read_db` takes `&PathBuf` instead of `&Path`

**File:** `src/tcc.rs:120`
**Severity:** NITPICK

```rust
fn read_db(path: &PathBuf, is_system: bool) -> Result<Vec<TccEntry>, String> {
```

Clippy would flag this: `&PathBuf` should be `&Path` per Rust convention (same as `&String` vs `&str`).

---

### 3.4 Duplicated auth_value → display string logic

**File:** `src/main.rs:111-115, 144-148, 150-155` and `src/tcc.rs:592-598`
**Severity:** MEDIUM

The auth_value → display name mapping (`0 → denied`, `2 → granted`, `3 → limited`, etc.) appears **three times** in `main.rs` (width calculation, plain text, colored text) and once more in `tcc.rs` (`auth_value_display`, gated behind `#[cfg(test)]`). If a new auth_value (like `5 = unknown(5)` visible in the README output) is given a name, you'd need to update 4 places.

**Fix:** Make `auth_value_display` non-test-only and use it from `print_entries`. For coloring, apply color to the returned string.

---

### 3.5 Massive `main()` match arms are identical boilerplate

**File:** `src/main.rs:353-447`
**Severity:** MEDIUM

Every subcommand (Grant, Revoke, Enable, Disable) follows the exact same pattern:
```rust
let db = TccDb::new(target);
match db.method(&service, &client_path) {
    Ok(msg) => println!("{}", msg.green()),
    Err(e) => { eprintln!("{}: {}", "Error".red().bold(), e); process::exit(1); }
}
```

This is repeated 5 times (6 including List). A helper function would eliminate the duplication:
```rust
fn run_command(result: Result<String, String>) {
    match result {
        Ok(msg) => println!("{}", msg.green()),
        Err(e) => {
            eprintln!("{}: {}", "Error".red().bold(), e);
            process::exit(1);
        }
    }
}
```

---

### 3.6 `TccEntry` fields are all `pub` with no encapsulation

**File:** `src/tcc.rs:59-66`
**Severity:** LOW

`TccEntry` is essentially a DTO — all fields are public, no invariants are maintained, and `service_display` / `last_modified` are pre-computed strings. This is fine for a two-file CLI, but the struct doesn't implement `Debug` or `Display`, making debugging harder. Consider `#[derive(Debug)]`.

---

### 3.7 `info()` is a static method that duplicates path construction

**File:** `src/tcc.rs:509-513`
**Severity:** LOW

`info()` reconstructs the DB paths (`home.join(...)`, `PathBuf::from(...)`) instead of using `self.user_db_path` / `self.system_db_path`. This means if the path logic ever changes in `new()`, `info()` would diverge. It should be an instance method using `&self`.

---

## 4. TYPE DESIGN

### 4.1 `DbTarget` could encode more intent

**File:** `src/tcc.rs:68-74`
**Severity:** LOW

`DbTarget::Default` means "both for reads, system for writes" — but this semantic is not obvious from the variant name. A name like `DbTarget::Auto` or `DbTarget::Both` would be clearer. Additionally, there's no `DbTarget::System` variant for "system only" (the `--user` flag gives User-only, but there's no `--system` flag for system-only).

---

### 4.2 No newtype for service keys

**File:** throughout `src/tcc.rs`
**Severity:** LOW

Service keys (`"kTCCServiceCamera"`) and display names (`"Camera"`) are both `String` / `&str`. It would be easy to accidentally pass a display name where a service key is expected. A newtype `ServiceKey(String)` would prevent this at compile time. Low priority for a CLI this size.

---

## 5. SQL & DATABASE

### 5.1 SQL queries are properly parameterized

**Severity:** POSITIVE (no issue)

All SQL queries use `?1`, `?2` etc. with `rusqlite::params![]`. No string interpolation into SQL. This is correct and safe from SQL injection.

---

### 5.2 `COALESCE(last_modified, auth_reason, 0)` conflates two different columns

**File:** `src/tcc.rs:129`
**Severity:** LOW

```sql
COALESCE(last_modified, auth_reason, 0) as modified
```

Falls back to `auth_reason` if `last_modified` is NULL. `auth_reason` is an integer enum (0=user, 1=system, 2=MDM, etc.), not a timestamp. If `last_modified` is NULL but `auth_reason` is, say, `3`, the code will interpret `3` as a CoreData timestamp (3 seconds after 2001-01-01), producing "2001-01-01 00:00:03". This won't crash but displays a misleading date.

**Fix:** Use `COALESCE(last_modified, 0)` — don't fall back to `auth_reason`.

---

### 5.3 No WAL mode or busy timeout for concurrent access

**File:** `src/tcc.rs:125-126, 306`
**Severity:** LOW

The tool opens the DB without setting a busy timeout. If macOS's TCC daemon has the DB locked (e.g., during a permission prompt), the tool will fail immediately with `SQLITE_BUSY`. Setting `conn.busy_timeout(Duration::from_secs(5))` would make it more robust.

---

## 6. TESTS

### 6.1 No tests for write operations

**File:** `tests/integration.rs`
**Severity:** MEDIUM

Integration tests only cover read-only commands (`list`, `services`, `info`). There are no tests for `grant`, `revoke`, `enable`, `disable`, or `reset`. These could be tested using an in-memory or temporary SQLite database.

---

### 6.2 Unit test `filter_entries` duplicates production logic

**File:** `src/tcc.rs:819-836`
**Severity:** LOW

The test helper `filter_entries` re-implements the filtering logic from `TccDb::list` rather than calling through the actual code. If the production logic changes, the test won't catch regressions.

---

### 6.3 `TccDb` is not testable in isolation

**File:** `src/tcc.rs:82-90`
**Severity:** MEDIUM

`TccDb::new()` hardcodes real DB paths. There's no way to construct a `TccDb` pointing at a test database without modifying `HOME`. A `TccDb::with_paths(user: PathBuf, system: PathBuf, target: DbTarget)` constructor would enable proper unit testing of all operations.

---

## 7. NITPICKS

### 7.1 `source_w` doesn't account for data

**File:** `src/main.rs:120`
**Severity:** NITPICK

```rust
let source_w = hdr_source.len();
```

This uses only the header width, not `max(header, max(data))`. In practice the data is always "user" or "system" (6 chars) and the header is "SOURCE" (6 chars), so it works, but it's inconsistent with how every other column width is calculated.

---

### 7.2 `FileProviderPresence` missing from `is_system_service`

**File:** `src/tcc.rs:232-242`
**Severity:** NITPICK

The `is_system_service` list is manually curated and may be incomplete or become stale as Apple adds new system-level services. This is inherently hard to keep up-to-date, but worth noting.

---

### 7.3 Missing `#[derive(Debug)]` on `TccEntry`

**File:** `src/tcc.rs:59`
**Severity:** NITPICK

`TccEntry` has no `Debug` derive, making it harder to use in test assertions or debug output. The struct is simple enough that deriving `Debug` has no downside.

---

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 0 |
| HIGH | 1 |
| MEDIUM | 8 |
| LOW | 7 |
| NITPICK | 4 |
| POSITIVE | 1 |

**Top priorities:**
1. **Fix silent row drops** (1.1) — most impactful correctness issue
2. **Fix ambiguous `resolve_service_name`** (1.2) — user-facing surprise
3. **Fix `client_type` in `grant`** (1.3) — functional correctness for bundle IDs
4. **Fix `reset` skipping root check** (1.4) — UX issue
5. **Fix `COALESCE` fallback to `auth_reason`** (5.2) — data correctness

Overall the codebase is well-structured, readable, and follows Rust idioms. SQL injection is properly prevented. The main issues are around edge-case handling and a few places where error information is lost or logic is duplicated.
