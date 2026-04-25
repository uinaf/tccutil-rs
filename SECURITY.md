# Security

If you believe you have found a security or privacy issue in this project, please report it privately.

## Contact

- Email: `dev@uinaf.dev`

Private reports are preferred. If you are unsure whether something is sensitive, email first instead of opening a public issue.

## Scope

Useful reports usually involve:

- Arbitrary write to a TCC database that bypasses the documented sudo / Full Disk Access requirements
- Privilege escalation through the CLI
- Input handling that lets an attacker grant or revoke permissions for an unintended client
- Service-name resolution flaws that route a write to the wrong database or the wrong row
- Credential, token, or secret leakage in logs, errors, or JSON output

Out of scope:

- macOS SIP blocking writes — the operating system enforcing its own policy.
- The user being prompted for Full Disk Access — Apple's TCC subsystem doing its job.
- Bugs in Apple's `tccutil`, `csrutil`, or SQLite. Report those upstream.

## Guidelines

- Test only against TCC databases on machines you control.
- Do not use real third-party bundle IDs or paths owned by others when reporting.
- Avoid destructive testing on production machines you depend on.

## Supported versions

Only the latest release receives security fixes. The project is maintained on a best-effort basis.

## Disclosure

Please allow a reasonable amount of time to investigate and fix before sharing details publicly. If the report is valid, we will work on a fix and coordinate disclosure.
