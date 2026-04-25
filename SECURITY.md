# security

if you believe you have found a security or privacy issue in this project, please report it privately.

## contact

- email: dev@uinaf.dev

private reports are preferred. if you are unsure whether something is sensitive, email first instead of opening a public issue.

## scope

useful reports usually involve:

- arbitrary write to a tcc database that bypasses the documented sudo / full disk access requirements
- privilege escalation through the cli
- input handling that lets an attacker grant or revoke permissions for an unintended client
- service-name resolution flaws that route a write to the wrong db or the wrong row
- credential, token, or secret leakage in logs, errors, or json output

out of scope:

- macos sip blocking writes — that is the operating system enforcing its own policy.
- the user being prompted for full disk access — that is apple's tcc subsystem doing its job.
- bugs in apple's `tccutil`, `csrutil`, or sqlite. report those upstream.

## guidelines

- test only against tcc databases on machines you control.
- do not use real third-party bundle ids or paths owned by others when reporting.
- avoid destructive testing on production machines you depend on.

## supported versions

only the latest release receives security fixes. the project is maintained on a best-effort basis.

## disclosure

please allow a reasonable amount of time to investigate and fix before sharing details publicly. if the report is valid, we will work on a fix and coordinate disclosure.
