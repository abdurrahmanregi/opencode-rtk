//! Integration tests for rtk-cli
//!
//! Tests the CLI behavior including:
//! - Exit codes (success, failure, not implemented)
//! - Input validation (empty, too large)
//! - Error messages

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_compress_success() {
    let mut cmd = Command::cargo_bin("rtk-cli").unwrap();

    cmd.arg("compress")
        .arg("-c")
        .arg("git status")
        .write_stdin("M file.rs\nA other.rs")
        .assert()
        .success();
}

#[test]
fn test_compress_empty_input() {
    let mut cmd = Command::cargo_bin("rtk-cli").unwrap();

    cmd.arg("compress")
        .arg("-c")
        .arg("git status")
        .write_stdin("")
        .assert()
        .failure()
        .stderr(predicate::str::contains("No input provided on stdin"));
}

#[test]
fn test_compress_missing_command() {
    let mut cmd = Command::cargo_bin("rtk-cli").unwrap();

    cmd.arg("compress")
        .write_stdin("some output")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--command"));
}

#[test]
fn test_health_daemon_not_running() {
    let mut cmd = Command::cargo_bin("rtk-cli").unwrap();

    // When daemon is not running, health check should fail with exit code 1
    cmd.arg("health")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("Failed to connect to daemon"));
}

#[test]
fn test_stats_not_implemented() {
    let mut cmd = Command::cargo_bin("rtk-cli").unwrap();

    cmd.arg("stats")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("not implemented"));
}

#[test]
fn test_stats_with_session_not_implemented() {
    let mut cmd = Command::cargo_bin("rtk-cli").unwrap();

    cmd.arg("stats")
        .arg("-s")
        .arg("session-123")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Requested session: session-123"));
}

#[test]
fn test_help_flag() {
    let mut cmd = Command::cargo_bin("rtk-cli").unwrap();

    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("CLI wrapper for RTK daemon"));
}

#[test]
fn test_compress_help() {
    let mut cmd = Command::cargo_bin("rtk-cli").unwrap();

    cmd.arg("compress")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Compress output via stdin"));
}
