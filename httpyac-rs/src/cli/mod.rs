//! Command-line interface for `httpyac-rs`.
//!
//! Phase 0 mirrors the upstream npm `httpyac` CLI surface flag-for-flag
//! and proxies every invocation through to the `httpyac` binary on
//! PATH. Later phases (1–7) will progressively replace the proxy with
//! native parsing, variable interpolation, HTTP execution, scripting,
//! and multi-protocol support — see
//! docs/superpowers/plans/2026-05-28-httpyac-rs-cli.md.

use clap::{Parser, Subcommand};

pub mod oauth2;
pub mod proxy;
pub mod send;

#[derive(Debug, Parser)]
#[command(
    name = "httpyac-rs",
    version,
    about = "HTTP/REST CLI Client for *.http files (Rust port of httpyac)",
    long_about = "httpyac-rs is a Rust reimplementation of the httpyac CLI. \
                  In its current state it forwards every command to the \
                  upstream `httpyac` binary on PATH; native execution is \
                  being added incrementally."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Send/execute http files.
    Send(send::SendArgs),
    /// Generate an OAuth2 token.
    Oauth2(oauth2::Oauth2Args),
}

pub async fn run() -> anyhow::Result<i32> {
    let cli = Cli::parse();
    match cli.command {
        Command::Send(args) => args.run().await,
        Command::Oauth2(args) => args.run().await,
    }
}
