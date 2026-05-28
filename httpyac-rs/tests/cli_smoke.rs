//! Smoke tests for the `httpyac-rs` CLI surface.
//!
//! These exercise clap's help/version/arg-parsing paths without ever
//! running npm `httpyac`, so they pass even on machines without
//! httpyac installed. Tests that exercise the actual proxy live in
//! ../tests/cli_proxy.rs and are gated behind the `httpyac` binary
//! being on PATH (TODO: add in a later phase when we want it).

#![cfg(feature = "cli")]

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

fn bin() -> Command {
    Command::cargo_bin("httpyac-rs").expect("httpyac-rs binary built")
}

#[test]
fn top_level_help_lists_subcommands() {
    bin()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("Usage: httpyac-rs"))
        .stdout(contains("send"))
        .stdout(contains("oauth2"));
}

#[test]
fn version_is_reported() {
    bin()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains("httpyac-rs"));
}

#[test]
fn send_help_lists_every_flag() {
    let assert = bin().args(["send", "--help"]).assert().success();
    let out = assert.get_output().stdout.clone();
    let stdout = String::from_utf8_lossy(&out);

    for flag in [
        "--all",
        "--bail",
        "--env",
        "--filter",
        "--insecure",
        "--interactive",
        "--json",
        "--junit",
        "--line",
        "--name",
        "--no-color",
        "--output",
        "--output-failed",
        "--raw",
        "--quiet",
        "--repeat",
        "--repeat-mode",
        "--parallel",
        "--silent",
        "--tag",
        "--timeout",
        "--var",
        "--verbose",
    ] {
        assert!(
            stdout.contains(flag),
            "send --help should list {flag}; got:\n{stdout}",
        );
    }
}

#[test]
fn oauth2_help_lists_every_flag() {
    let assert = bin().args(["oauth2", "--help"]).assert().success();
    let out = assert.get_output().stdout.clone();
    let stdout = String::from_utf8_lossy(&out);

    for flag in ["--flow", "--prefix", "--env", "--output", "--var"] {
        assert!(
            stdout.contains(flag),
            "oauth2 --help should list {flag}; got:\n{stdout}",
        );
    }
}

#[test]
fn send_requires_a_file_argument() {
    bin()
        .arg("send")
        .assert()
        .failure()
        .stderr(contains("required").or(contains("FILE")));
}

#[test]
fn send_rejects_unknown_flag() {
    bin()
        .args(["send", "--definitely-not-a-real-flag", "x.http"])
        .assert()
        .failure()
        .stderr(contains("unexpected").or(contains("unknown")));
}

#[test]
fn send_rejects_unknown_output_value() {
    bin()
        .args(["send", "--output", "tornado", "x.http"])
        .assert()
        .failure()
        .stderr(contains("invalid value").or(contains("possible values")));
}

#[test]
fn send_accepts_multi_value_env_and_var() {
    // Doesn't run httpyac — just verifies clap accepts the form
    // without `--` between groups. We catch this at the parse level
    // by passing a bogus output value AFTER the multi-value list, so
    // clap parses everything and only then errors on the output.
    bin()
        .args([
            "send",
            "--env", "local", "staging",
            "--var", "foo=1", "bar=2",
            "--output", "tornado",  // forces a parse-level failure
            "x.http",
        ])
        .assert()
        .failure()
        .stderr(contains("invalid value").or(contains("possible values")));
}
