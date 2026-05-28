//! `httpyac-rs send` — mirrors the flag surface of `httpyac send`.
//!
//! Phase 0 forwards every argument to the upstream `httpyac` CLI on
//! PATH; the flag surface here exists so the binary is a drop-in alias
//! and so later phases can swap the proxy out flag-by-flag without
//! breaking callers.

use std::ffi::OsString;

use anyhow::Result;
use clap::{Args, ValueEnum};

use crate::cli::proxy::proxy_to_httpyac;

#[derive(Debug, Args)]
pub struct SendArgs {
    /// Path(s) or glob pattern(s) to .http file(s) to execute.
    #[arg(required = true, value_name = "FILE")]
    pub file_name: Vec<String>,

    /// Execute all HTTP requests in the file.
    #[arg(short = 'a', long)]
    pub all: bool,

    /// Stop when a test case fails.
    #[arg(long)]
    pub bail: bool,

    /// One or more environments to apply (httpyac env files).
    #[arg(short = 'e', long, num_args = 1..)]
    pub env: Vec<String>,

    /// Filter the response output (e.g. `only-failed`).
    #[arg(long, value_name = "FILTER")]
    pub filter: Option<String>,

    /// Allow insecure server connections (skip TLS verification).
    #[arg(long)]
    pub insecure: bool,

    /// Stay in interactive selection mode after each request.
    #[arg(short = 'i', long)]
    pub interactive: bool,

    /// Emit JSON output (machine-readable).
    #[arg(long)]
    pub json: bool,

    /// Emit JUnit XML output.
    #[arg(long)]
    pub junit: bool,

    /// Execute the request at the given line number (1-indexed).
    #[arg(short = 'l', long, value_name = "LINE")]
    pub line: Option<u32>,

    /// Execute the request with the given `@name`.
    #[arg(short = 'n', long, value_name = "NAME")]
    pub name: Option<String>,

    /// Disable color in stdout.
    #[arg(long = "no-color")]
    pub no_color: bool,

    /// Output format for successful responses.
    #[arg(short = 'o', long, value_enum)]
    pub output: Option<OutputFormat>,

    /// Output format for failed responses.
    #[arg(long = "output-failed", value_enum)]
    pub output_failed: Option<OutputFormat>,

    /// Disable body formatting (return raw bytes).
    #[arg(long)]
    pub raw: bool,

    /// Suppress non-essential output.
    #[arg(long)]
    pub quiet: bool,

    /// Repeat each request N times.
    #[arg(long, value_name = "COUNT")]
    pub repeat: Option<u32>,

    /// How repeats are scheduled.
    #[arg(long = "repeat-mode", value_enum)]
    pub repeat_mode: Option<RepeatMode>,

    /// Send N requests in parallel.
    #[arg(long, value_name = "COUNT")]
    pub parallel: Option<u32>,

    /// Log only the request line, not the response.
    #[arg(short = 's', long)]
    pub silent: bool,

    /// Execute only requests tagged with the given tag(s).
    #[arg(short = 't', long, num_args = 1..)]
    pub tag: Vec<String>,

    /// Maximum connection time in milliseconds.
    #[arg(long, value_name = "MS")]
    pub timeout: Option<u32>,

    /// Define one or more variables, e.g. `--var foo=bar baz=qux`.
    #[arg(long, num_args = 1..)]
    pub var: Vec<String>,

    /// Verbose output.
    #[arg(short = 'v', long)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Short,
    Body,
    Headers,
    Response,
    Exchange,
    None,
}

impl OutputFormat {
    fn as_cli_str(self) -> &'static str {
        match self {
            OutputFormat::Short => "short",
            OutputFormat::Body => "body",
            OutputFormat::Headers => "headers",
            OutputFormat::Response => "response",
            OutputFormat::Exchange => "exchange",
            OutputFormat::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RepeatMode {
    Sequential,
    Parallel,
}

impl RepeatMode {
    fn as_cli_str(self) -> &'static str {
        match self {
            RepeatMode::Sequential => "sequential",
            RepeatMode::Parallel => "parallel",
        }
    }
}

impl SendArgs {
    /// Re-serialize the parsed args into the argv form `httpyac send`
    /// expects. The order doesn't matter to httpyac but we keep it
    /// stable for snapshot-test friendliness.
    pub fn to_argv(&self) -> Vec<OsString> {
        let mut argv: Vec<OsString> = Vec::new();

        for f in &self.file_name {
            argv.push(f.into());
        }

        if self.all {
            argv.push("--all".into());
        }
        if self.bail {
            argv.push("--bail".into());
        }
        for e in &self.env {
            argv.push("--env".into());
            argv.push(e.into());
        }
        if let Some(filter) = &self.filter {
            argv.push("--filter".into());
            argv.push(filter.into());
        }
        if self.insecure {
            argv.push("--insecure".into());
        }
        if self.interactive {
            argv.push("--interactive".into());
        }
        if self.json {
            argv.push("--json".into());
        }
        if self.junit {
            argv.push("--junit".into());
        }
        if let Some(line) = self.line {
            argv.push("--line".into());
            argv.push(line.to_string().into());
        }
        if let Some(name) = &self.name {
            argv.push("--name".into());
            argv.push(name.into());
        }
        if self.no_color {
            argv.push("--no-color".into());
        }
        if let Some(output) = self.output {
            argv.push("--output".into());
            argv.push(output.as_cli_str().into());
        }
        if let Some(output_failed) = self.output_failed {
            argv.push("--output-failed".into());
            argv.push(output_failed.as_cli_str().into());
        }
        if self.raw {
            argv.push("--raw".into());
        }
        if self.quiet {
            argv.push("--quiet".into());
        }
        if let Some(repeat) = self.repeat {
            argv.push("--repeat".into());
            argv.push(repeat.to_string().into());
        }
        if let Some(mode) = self.repeat_mode {
            argv.push("--repeat-mode".into());
            argv.push(mode.as_cli_str().into());
        }
        if let Some(parallel) = self.parallel {
            argv.push("--parallel".into());
            argv.push(parallel.to_string().into());
        }
        if self.silent {
            argv.push("--silent".into());
        }
        for t in &self.tag {
            argv.push("--tag".into());
            argv.push(t.into());
        }
        if let Some(timeout) = self.timeout {
            argv.push("--timeout".into());
            argv.push(timeout.to_string().into());
        }
        for v in &self.var {
            argv.push("--var".into());
            argv.push(v.into());
        }
        if self.verbose {
            argv.push("--verbose".into());
        }

        argv
    }

    pub async fn run(self) -> Result<i32> {
        proxy_to_httpyac("send", self.to_argv()).await
    }
}
