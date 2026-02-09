# tcc-cli

A macOS CLI for viewing and managing TCC (Transparency, Consent, and Control) permissions.

## Problem

macOS tracks privacy permissions (camera, microphone, screen recording, accessibility, etc.) in a SQLite database called TCC. The System Settings Privacy & Security UI only shows `.app` bundles — CLI tools, scripts, and other non-app binaries that hold permissions are invisible. `tcc-cli` lets you see and manage all of them.

## Installation

```
cargo build --release
cp target/release/tcc ~/.local/bin/   # or anywhere on your PATH
```

## Usage

### List all permissions

```
$ tcc list
SERVICE              CLIENT                                    STATUS   SOURCE  LAST MODIFIED
Accessibility        /usr/local/bin/cliclick                   granted  system  2025-08-14 10:22:03
Camera               com.google.Chrome                         granted  user    2025-06-01 09:15:42
Full Disk Access     com.apple.Terminal                         granted  user    2025-07-20 14:30:11
Screen Recording     com.loom.desktop                          granted  system  2025-09-03 16:45:00

4 entries
```

### Filter by client

```
$ tcc list --client node
SERVICE              CLIENT                                    STATUS   SOURCE  LAST MODIFIED
Full Disk Access     /usr/local/bin/node                        granted  user    2025-05-12 08:00:33

1 entry
```

### Filter by service

```
$ tcc list --service "Screen"
SERVICE              CLIENT                                    STATUS   SOURCE  LAST MODIFIED
Screen Recording     com.loom.desktop                          granted  system  2025-09-03 16:45:00
Screen Recording     com.apple.Terminal                         denied   system  2025-10-01 11:20:00

2 entries
```

### List known service names

```
$ tcc services
INTERNAL NAME                              DESCRIPTION
kTCCServiceAccessibility                   Accessibility
kTCCServiceAddressBook                     Address Book
kTCCServiceCalendar                        Calendar
kTCCServiceCamera                          Camera
...
```

### Grant a permission

```
$ sudo tcc grant Accessibility /usr/local/bin/my-tool
Granted Accessibility to /usr/local/bin/my-tool (system database)
```

Accepts internal names (`kTCCServiceCamera`) or display names (`Camera`). System services (Accessibility, Screen Recording, etc.) require `sudo`.

### Revoke a permission

```
$ sudo tcc revoke Accessibility /usr/local/bin/my-tool
Revoked Accessibility from /usr/local/bin/my-tool (system database)
```

## SIP limitations

On macOS 10.14+, System Integrity Protection restricts direct writes to TCC databases. Some operations may fail even with `sudo` if SIP is enabled. Read operations (listing permissions) work without issue. For write operations that SIP blocks, you would need to either disable SIP (not recommended) or use `tccutil` for the limited operations it supports.

## License

MIT
