use std::process::Command;

/// Helper: run the `tccutil-rs` binary with given args, returning (stdout, stderr, success).
fn run_tcc(args: &[&str]) -> (String, String, bool) {
    let bin = env!("CARGO_BIN_EXE_tccutil-rs");
    let output = Command::new(bin)
        .args(args)
        .output()
        .expect("failed to execute tccutil-rs binary");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

// ── tccutil-rs services ─────────────────────────────────────────────

#[test]
fn services_runs_and_lists_known_services() {
    let (stdout, _stderr, success) = run_tcc(&["services"]);
    assert!(success, "tccutil-rs services should exit 0");

    // Header row
    assert!(stdout.contains("INTERNAL NAME"), "should have header");
    assert!(
        stdout.contains("DESCRIPTION"),
        "should have description header"
    );

    // Spot-check a handful of well-known service names
    assert!(
        stdout.contains("Accessibility"),
        "should list Accessibility"
    );
    assert!(stdout.contains("Camera"), "should list Camera");
    assert!(stdout.contains("Microphone"), "should list Microphone");
    assert!(
        stdout.contains("Screen Recording"),
        "should list Screen Recording"
    );
    assert!(
        stdout.contains("Full Disk Access"),
        "should list Full Disk Access"
    );
}

// ── tccutil-rs list ─────────────────────────────────────────────────

#[test]
fn list_runs_without_error() {
    // list reads the user TCC DB — may return entries or "No entries found."
    // Either way it should not crash.
    let (stdout, _stderr, success) = run_tcc(&["--user", "list"]);
    assert!(success, "tccutil-rs --user list should exit 0");
    // Output is either the table or the empty-state message
    assert!(
        stdout.contains("SERVICE") || stdout.contains("No entries found"),
        "expected table header or empty message, got: {}",
        stdout
    );
}

#[test]
fn list_compact_runs_without_error() {
    let (_stdout, _stderr, success) = run_tcc(&["--user", "list", "--compact"]);
    assert!(success, "tccutil-rs --user list --compact should exit 0");
}

#[test]
fn list_with_client_filter_runs() {
    let (_stdout, _stderr, success) = run_tcc(&["--user", "list", "--client", "apple"]);
    assert!(
        success,
        "tccutil-rs --user list --client apple should exit 0"
    );
}

#[test]
fn list_with_service_filter_runs() {
    let (_stdout, _stderr, success) = run_tcc(&["--user", "list", "--service", "Camera"]);
    assert!(
        success,
        "tccutil-rs --user list --service Camera should exit 0"
    );
}

// ── tccutil-rs info ─────────────────────────────────────────────────

#[test]
fn info_shows_macos_version_and_db_paths() {
    let (stdout, _stderr, success) = run_tcc(&["info"]);
    assert!(success, "tccutil-rs info should exit 0");

    assert!(
        stdout.contains("macOS version:"),
        "should show macOS version"
    );
    assert!(stdout.contains("User DB:"), "should show User DB path");
    assert!(stdout.contains("System DB:"), "should show System DB path");
    assert!(stdout.contains("TCC.db"), "should mention TCC.db");
    assert!(stdout.contains("SIP status:"), "should show SIP status");
}

// ── Error cases ──────────────────────────────────────────────────────

#[test]
fn no_subcommand_prints_help_and_fails() {
    let (_stdout, stderr, success) = run_tcc(&[]);
    assert!(!success, "tccutil-rs with no args should fail");
    // clap prints usage to stderr
    assert!(
        stderr.contains("Usage") || stderr.contains("usage"),
        "should print usage info"
    );
}

#[test]
fn unknown_subcommand_fails() {
    let (_stdout, _stderr, success) = run_tcc(&["bogus"]);
    assert!(!success, "tccutil-rs bogus should fail");
}

#[test]
fn version_flag_prints_version() {
    let (stdout, _stderr, success) = run_tcc(&["--version"]);
    assert!(success, "tccutil-rs --version should exit 0");
    assert!(
        stdout.contains("tccutil-rs"),
        "version output should mention tccutil-rs"
    );
}
