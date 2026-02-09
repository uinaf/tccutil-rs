# tccutil (Rust)

A Rust replacement for Apple's `/usr/bin/tccutil` and [jacobsalmela/tccutil.py](https://github.com/jacobsalmela/tccutil). Zero runtime dependencies, single static binary.

Binary name: **`tccutil-rs`** (to avoid clashing with Apple's built-in `/usr/bin/tccutil`).

## The problem

macOS tracks privacy permissions (Camera, Microphone, Screen Recording, Accessibility, etc.) in a SQLite database called TCC (Transparency, Consent, and Control). Two issues make this painful to manage:

1. **System Settings only shows `.app` bundles.** CLI tools, scripts, and other non-app binaries that hold permissions are invisible in the Privacy & Security UI. You can't see or manage them.
2. **Apple's `tccutil` only supports `reset`.** You can wipe all entries for a service, but you can't list, grant, revoke, enable, or disable individual entries.

`tccutil-rs` gives you full read/write access to both the user and system TCC databases.

## Install

### One-liner

```sh
curl -sSL https://raw.githubusercontent.com/uinafdev/tccutil/master/install.sh | sh
```

### From source

```sh
cargo build --release
cp target/release/tccutil-rs /usr/local/bin/
```

### Shell alias (optional)

```sh
# Add to ~/.zshrc or ~/.bashrc
# Note: overrides Apple's tccutil, which only has `reset`
alias tccutil="tccutil-rs"
```

## Commands

### `tccutil-rs list` — List all permissions

Shows all TCC entries from both user and system databases.

```
$ tccutil-rs list --compact
SERVICE                    CLIENT                                                    STATUS      SOURCE  LAST MODIFIED
─────────────────────────  ────────────────────────────────────────────────────────  ──────────  ──────  ───────────────────
Accessibility              node                                                      granted     system  2026-02-09 12:10:33
Accessibility              com.1password.1password                                   denied      system  2026-02-02 17:48:19
Accessibility              com.raycast.macos                                         granted     system  2026-02-03 11:58:50
Apple Events / Automation  node                                                      granted     user    2026-02-08 20:16:08
Full Disk Access           op                                                        granted     system  2026-02-03 08:11:05
Full Disk Access           node                                                      granted     system  2026-02-03 20:58:23
Screen Recording           node                                                      granted     system  2026-02-09 11:31:33
...
```

#### `--client <NAME>` — Filter by client (partial match)

```
$ tccutil-rs list --client node --compact
SERVICE                    CLIENT  STATUS      SOURCE  LAST MODIFIED
─────────────────────────  ──────  ──────────  ──────  ───────────────────
Accessibility              node    granted     system  2026-02-09 12:10:33
Apple Events / Automation  ″       granted     user    2026-02-08 20:16:08
Downloads Folder           ″       granted     user    2026-02-02 21:03:55
File Provider              ″       granted     user    2026-02-02 21:18:13
Full Disk Access           ″       granted     system  2026-02-03 20:58:23
Reminders                  ″       granted     user    2026-02-02 22:05:13
Screen Recording           ″       granted     system  2026-02-09 11:31:33

9 entries total
```

#### `--service <NAME>` — Filter by service

```
$ tccutil-rs list --service "Screen Recording"
SERVICE           CLIENT                                         STATUS   SOURCE  LAST MODIFIED
────────────────  ─────────────────────────────────────────────  ───────  ──────  ───────────────────
Screen Recording  /opt/homebrew/Cellar/node@22/22.22.0/bin/node  granted  system  2026-02-09 11:31:33
Screen Recording  com.apple.screensharing.agent                  granted  system  2026-02-02 21:56:30

2 entries total
```

#### `--user` — Query user database only

By default, `tccutil-rs` reads both databases and shows a source column. Use `--user` to query only the per-user database.

### `tccutil-rs services` — List known TCC service names

Maps internal `kTCCService*` identifiers to human-readable names. Both forms are accepted by all commands.

```
$ tccutil-rs services
INTERNAL NAME                        DESCRIPTION
───────────────────────────────────  ─────────────────────────
kTCCServiceAccessibility             Accessibility
kTCCServiceAddressBook               Address Book
kTCCServiceAppleEvents               Apple Events / Automation
kTCCServiceCalendar                  Calendar
kTCCServiceCamera                    Camera
kTCCServiceScreenCapture             Screen Recording
kTCCServiceSystemPolicyAllFiles      Full Disk Access
...
```

### `tccutil-rs info` — Show database info and SIP status

```
$ tccutil-rs info
macOS version: 26.2
SIP status: System Integrity Protection status: enabled.

User DB: /Users/glitch/Library/Application Support/com.apple.TCC/TCC.db
  Readable: yes
  Writable: yes
  Schema digest: 34abf99d20 (known)

System DB: /Library/Application Support/com.apple.TCC/TCC.db
  Readable: yes
  Writable: yes
  Schema digest: 34abf99d20 (known)
```

### `tccutil-rs grant` — Grant a permission

```
$ sudo tccutil-rs grant Accessibility /usr/local/bin/my-tool
Granted Accessibility to /usr/local/bin/my-tool (system database)
```

System-level services require `sudo`. Use `--user` to write to the user database instead.

### `tccutil-rs revoke` — Revoke a permission

```
$ sudo tccutil-rs revoke Accessibility /usr/local/bin/my-tool
Revoked Accessibility from /usr/local/bin/my-tool (system database)
```

### `tccutil-rs enable` / `disable` — Toggle an existing entry

```
$ sudo tccutil-rs enable Accessibility /usr/local/bin/my-tool
Enabled Accessibility for /usr/local/bin/my-tool (system database)

$ sudo tccutil-rs disable Accessibility /usr/local/bin/my-tool
Disabled Accessibility for /usr/local/bin/my-tool (system database)
```

### `tccutil-rs reset` — Reset entries for a service

```
$ sudo tccutil-rs reset Accessibility
Reset all entries for Accessibility (system database)

$ sudo tccutil-rs reset Accessibility /usr/local/bin/my-tool
Reset Accessibility for /usr/local/bin/my-tool (system database)
```

## Global flags

| Flag | Description |
|------|-------------|
| `--user`, `-u` | Operate on the per-user database instead of the system database |
| `--compact` | Show binary names instead of full paths (list only) |
| `--help`, `-h` | Print help |
| `--version`, `-V` | Print version |

## SIP limitations

On macOS 10.14+, System Integrity Protection restricts direct writes to TCC databases. Read operations (`list`, `services`, `info`) always work. Write operations (`grant`, `revoke`, `enable`, `disable`, `reset`) may fail even with `sudo` if SIP is enabled.

In practice, the **user database** is writable regardless of SIP. The **system database** requires running with `sudo` (works for most operations on recent macOS).

## Comparison

| | Apple `tccutil` | [tccutil.py](https://github.com/jacobsalmela/tccutil) | `tccutil-rs` |
|---|---|---|---|
| Language | Built-in (Obj-C) | Python | Rust |
| Dependencies | Ships with macOS | Python 3 | None (static binary) |
| List permissions | ❌ | ✅ | ✅ |
| Filter by client/service | ❌ | ✅ | ✅ |
| Compact output | ❌ | ❌ | ✅ |
| Grant | ❌ | ✅ | ✅ |
| Revoke | ❌ | ✅ | ✅ |
| Enable/Disable toggle | ❌ | ❌ | ✅ |
| Reset | ✅ (only feature) | ✅ | ✅ |
| Service name lookup | ❌ | ❌ | ✅ |
| DB info / SIP check | ❌ | ❌ | ✅ |
| User + System DB | System only | Both | Both |
| macOS version support | Current | 10.9–14 | 15+ |

## License

[MIT](LICENSE)
