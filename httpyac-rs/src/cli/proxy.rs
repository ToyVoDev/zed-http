//! Shared helpers for proxying CLI invocations to the upstream `httpyac`
//! binary on PATH. Used by every phase 0 subcommand; will be removed
//! once native implementations land in later phases.

use std::ffi::OsString;
use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::process::Command;

/// Spawn `httpyac <subcommand> <args...>` inheriting the current
/// process's stdio so the user sees httpyac's output unmodified.
/// Returns httpyac's exit code (or 1 if it was killed by a signal).
pub async fn proxy_to_httpyac(subcommand: &str, args: Vec<OsString>) -> Result<i32> {
    let mut cmd = Command::new("httpyac");
    cmd.arg(subcommand)
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = cmd.status().await.with_context(|| {
        "failed to spawn `httpyac`. Install it with `npm install -g httpyac` \
         (or via your package manager) and ensure it is on PATH."
    })?;

    Ok(status.code().unwrap_or(1))
}
