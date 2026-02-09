# tcc-cli

A Rust replacement for Apple's `tccutil` and [jacobsalmela/tccutil.py](https://github.com/jacobsalmela/tccutil). Zero runtime dependencies, single static binary.

## The problem

macOS tracks privacy permissions (Camera, Microphone, Screen Recording, Accessibility, etc.) in a SQLite database called TCC (Transparency, Consent, and Control). Two issues make this painful to manage:

1. **System Settings only shows `.app` bundles.** CLI tools, scripts, and other non-app binaries that hold permissions are invisible in the Privacy & Security UI. You can't see or manage them.
2. **Apple's `tccutil` only supports `reset`.** You can wipe all entries for a service, but you can't list, grant, revoke, enable, or disable individual entries.

`tcc-cli` gives you full read/write access to both the user and system TCC databases.

## Installation

```
cargo build --release
cp target/release/tcc ~/.local/bin/   # or anywhere on your PATH
```

## Commands

### `tcc list` -- List all permissions

Shows all TCC entries from the system database by default.

```
$ tcc list
SERVICE                    CLIENT                                                                                      STATUS      SOURCE    LAST MODIFIED
─────────────────────────  ──────────────────────────────────────────────────────────────────────────────────────────  ──────────  ────────  ─────────────
Accessibility              /opt/homebrew/Cellar/node@22/22.22.0/bin/node                                               granted     system    2026-02-09 12:10:33
Accessibility              com.1password.1password                                                                     denied      system    2026-02-02 17:48:19
Accessibility              com.raycast.macos                                                                           granted     system    2026-02-03 11:58:50
Apple Events / Automation  /opt/homebrew/Cellar/node@22/22.22.0/bin/node                                               granted     user      2026-02-08 20:16:08
Full Disk Access           /opt/homebrew/Caskroom/1password-cli/2.32.0/op                                              granted     system    2026-02-03 08:11:05
Full Disk Access           /opt/homebrew/Cellar/node@22/22.22.0/bin/node                                               granted     system    2026-02-03 20:58:23
Screen Recording           /opt/homebrew/Cellar/node@22/22.22.0/bin/node                                               granted     system    2026-02-09 11:31:33
Screen Recording           com.apple.screensharing.agent                                                               granted     system    2026-02-02 21:56:30
...
```

#### `--compact` -- Show binary names instead of full paths

```
$ tcc list --compact
SERVICE                    CLIENT                                                    STATUS      SOURCE    LAST MODIFIED
─────────────────────────  ────────────────────────────────────────────────────────  ──────────  ────────  ─────────────
Accessibility              node                                                      granted     system    2026-02-09 12:10:33
Accessibility              com.1password.1password                                   denied      system    2026-02-02 17:48:19
Accessibility              com.raycast.macos                                         granted     system    2026-02-03 11:58:50
Full Disk Access           siriactionsd                                              denied      system    2026-02-02 15:52:03
Full Disk Access           op                                                        granted     system    2026-02-03 08:11:05
Full Disk Access           bun                                                       denied      system    2026-02-04 08:40:18
Full Disk Access           node                                                      granted     system    2026-02-03 20:58:23
...
```

#### `--client <NAME>` -- Filter by client (partial match)

```
$ tcc list --client node --compact
SERVICE                    CLIENT      STATUS      SOURCE    LAST MODIFIED
─────────────────────────  ──────────  ──────────  ────────  ─────────────
Accessibility              node        granted     system    2026-02-09 12:10:33
Apple Events / Automation  node        granted     user      2026-02-08 20:16:08
Downloads Folder           node        granted     user      2026-02-02 21:03:55
File Provider              node        granted     user      2026-02-02 21:18:13
Full Disk Access           node        granted     system    2026-02-03 20:58:23
Reminders                  node        granted     user      2026-02-02 22:05:13
Screen Recording           node        granted     system    2026-02-09 11:31:33
SystemPolicyAppBundles     node        denied      user      2026-02-02 20:56:47
SystemPolicyAppData        node        unknown(5)  user      2026-02-03 20:54:53

9 entries total
```

#### `--service <NAME>` -- Filter by service (partial match)

```
$ tcc list --service "Screen"
SERVICE           CLIENT                                         STATUS      SOURCE    LAST MODIFIED
────────────────  ─────────────────────────────────────────────  ──────────  ────────  ─────────────
Screen Recording  /opt/homebrew/Cellar/node@22/22.22.0/bin/node  granted     system    2026-02-09 11:31:33
Screen Recording  com.apple.screensharing.agent                  granted     system    2026-02-02 21:56:30

2 entries total
```

#### `--user` -- Query the user database

By default `tcc` reads both databases and shows the source column. Use `--user` to read only the per-user database.

```
$ tcc list --user --compact
SERVICE                    CLIENT                                                    STATUS      SOURCE    LAST MODIFIED
─────────────────────────  ────────────────────────────────────────────────────────  ──────────  ────────  ─────────────
Apple Events / Automation  node                                                      granted     user      2026-02-08 20:16:08
Calendar                   com.raycast.macos                                         granted     user      2026-02-03 11:58:25
Desktop Folder             com.raycast.macos                                         granted     user      2026-02-03 11:58:21
Downloads Folder           node                                                      granted     user      2026-02-02 21:03:55
Downloads Folder           com.raycast.macos                                         granted     user      2026-02-03 11:58:23
...
```

### `tcc services` -- List known TCC service names

Maps internal `kTCCService*` identifiers to human-readable names. Both forms are accepted by all commands.

```
$ tcc services
INTERNAL NAME                        DESCRIPTION
───────────────────────────────────  ─────────────────────────
kTCCServiceAccessibility             Accessibility
kTCCServiceAddressBook               Address Book
kTCCServiceSystemPolicySysAdminFiles  Administer Computer (SysAdmin)
kTCCServiceAppleEvents               Apple Events / Automation
kTCCServiceBluetoothAlways           Bluetooth
kTCCServiceCalendar                  Calendar
kTCCServiceCamera                    Camera
kTCCServiceContacts                  Contacts
kTCCServiceSystemPolicyDesktopFolder  Desktop Folder
kTCCServiceDeveloperTool             Developer Tool
kTCCServiceSystemPolicyDocumentsFolder  Documents Folder
kTCCServiceSystemPolicyDownloadsFolder  Downloads Folder
kTCCServiceEndpointSecurityClient    Endpoint Security
kTCCServiceFileProviderDomain        File Provider
kTCCServiceFocusStatus               Focus Status
kTCCServiceSystemPolicyAllFiles      Full Disk Access
kTCCServiceListenEvent               Input Monitoring
kTCCServiceLocation                  Location
kTCCServiceMediaLibrary              Media Library
kTCCServiceMicrophone                Microphone
kTCCServiceSystemPolicyNetworkVolumes  Network Volumes
kTCCServicePhotos                    Photos
kTCCServicePhotosAdd                 Photos (Add Only)
kTCCServicePostEvent                 Post Events
kTCCServiceReminders                 Reminders
kTCCServiceSystemPolicyRemovableVolumes  Removable Volumes
kTCCServiceScreenCapture             Screen Recording
kTCCServiceSpeechRecognition         Speech Recognition
kTCCServiceLiverpool                 User Data (Liverpool)
```

### `tcc info` -- Show database info and SIP status

```
$ tcc info
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

### `tcc grant` -- Grant a permission

Inserts a new TCC entry. Accepts display names (`Accessibility`) or internal names (`kTCCServiceAccessibility`).

```
$ sudo tcc grant Accessibility /usr/local/bin/my-tool
Granted Accessibility to /usr/local/bin/my-tool (system database)
```

System-level services (Accessibility, Screen Recording, Full Disk Access, etc.) require `sudo`. Use `--user` to write to the user database instead.

### `tcc revoke` -- Revoke a permission

Deletes the TCC entry entirely.

```
$ sudo tcc revoke Accessibility /usr/local/bin/my-tool
Revoked Accessibility from /usr/local/bin/my-tool (system database)
```

### `tcc enable` -- Enable an existing permission

Sets `auth_value=2` on an existing entry (flips it to granted without deleting/recreating).

```
$ sudo tcc enable Accessibility /usr/local/bin/my-tool
Enabled Accessibility for /usr/local/bin/my-tool (system database)
```

### `tcc disable` -- Disable an existing permission

Sets `auth_value=0` on an existing entry (flips it to denied).

```
$ sudo tcc disable Accessibility /usr/local/bin/my-tool
Disabled Accessibility for /usr/local/bin/my-tool (system database)
```

### `tcc reset` -- Reset entries for a service

Deletes all entries for a service, or a specific client within a service.

```
$ sudo tcc reset Accessibility
Reset all entries for Accessibility (system database)

$ sudo tcc reset Accessibility /usr/local/bin/my-tool
Reset Accessibility for /usr/local/bin/my-tool (system database)
```

## Global flags

| Flag | Description |
|------|-------------|
| `--user`, `-u` | Operate on the per-user database instead of the system database |
| `--help`, `-h` | Print help |
| `--version`, `-V` | Print version |

## SIP limitations

On macOS 10.14+, System Integrity Protection restricts direct writes to TCC databases. Read operations (`list`, `services`, `info`) always work. Write operations (`grant`, `revoke`, `enable`, `disable`, `reset`) may fail even with `sudo` if SIP is enabled.

In practice, the **user database** is writable regardless of SIP. The **system database** requires either:
- Running with `sudo` (works for most operations on recent macOS)
- Disabling SIP (not recommended for production machines)

## Comparison

| | Apple `tccutil` | [tccutil.py](https://github.com/jacobsalmela/tccutil) | `tcc-cli` |
|---|---|---|---|
| Language | Built-in (Obj-C) | Python | Rust |
| Dependencies | None (ships with macOS) | Python 3 | None (static binary) |
| List permissions | No | Yes | Yes |
| Filter by client/service | No | Yes | Yes |
| Compact output | No | No | Yes |
| Grant | No | Yes | Yes |
| Revoke | No | Yes | Yes |
| Enable/Disable | No | No | Yes |
| Reset | Yes (only feature) | Yes | Yes |
| Service name lookup | No | No | Yes |
| Database info / SIP check | No | No | Yes |
| User + System DB | System only | Both | Both |

## License

MIT
