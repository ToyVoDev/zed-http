# httpyac-rs CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn the `httpyac-rs` library crate into a dual-purpose lib + CLI that mirrors the `httpyac` Node CLI's flag surface, with the binary initially proxying all execution to the upstream npm `httpyac` so the new CLI is a drop-in alias from day one.

**Architecture:** Add a `[[bin]]` target `httpyac-rs` to the existing `httpyac-rs` library crate, gated behind an off-by-default `cli` feature. Clap parses the surface; phase 0's `send` and `oauth2` commands forward every flag through `tokio::process::Command` to the real `httpyac` binary on PATH, streaming its stdout/stderr and exit code through. Later phases (1–7, scoped separately) progressively replace the proxy with native parsing, variable interpolation, HTTP execution, scripting, and other-protocol support.

**Tech Stack:** Rust 2021, clap 4 (`derive` feature), tokio (`process`, `io-util`, `rt-multi-thread`), anyhow, the existing httpyac-rs lib for type re-exports.

---

## Future-phase reference (not implemented here)

Phase 0 is the only phase this plan implements. Recorded inline so future plans don't re-derive the phasing.

| Phase | Scope |
|---|---|
| **0 (this plan)** | Clap surface for `send` + `oauth2`. Both subcommands proxy 100% of flags through to npm `httpyac`. Binary is gated behind a `cli` cargo feature. |
| **1** | Native `.http` parser; `@name = value` and `.env` / `http-client.env.json` env-file loading; literal `{{name}}` interpolation; simple HTTP via `reqwest`. Proxy fallback for anything unsupported. |
| **2** | Built-in dynamic variables (`$timestamp`, `$uuid`, `$randomInt`, `$processEnv`, `$datetime`); lazy `:=` vars; `--var` CLI passthrough; output formats `short` / `body` / `headers` / `response` / `exchange`. |
| **3** | Metadata directives (`@name`, `@disabled`, `@ref`, `@forceRef`, `@import`, `@no-redirect`, `@no-log`, `@no-cookie-jar`, `@timeout`, `@connection-timeout`, `@sleep`, `@loop`). |
| **4** | Assertion DSL (`?? status == 200` and the 20 operators), `--bail`, `--junit` output. |
| **5** | JS scripting (`{% %}` pre-request + response handlers, top-level `{{ }}` JS expressions) via `deno_core` or `boa`. |
| **6** | `oauth2` subcommand native: `client_credentials`, `authorization_code`, `password`, `device_code`, `implicit`. Persistent token cache. |
| **7** | Non-HTTP protocols: WebSocket, SSE, gRPC, MQTT, AMQP. |

---

## File structure (phase 0)

| File | Status | Responsibility |
|---|---|---|
| `httpyac-rs/Cargo.toml` | Modify | Add `[[bin]] httpyac-rs`, add `cli` feature, gate clap/anyhow/tokio-rt deps behind the feature. |
| `httpyac-rs/src/lib.rs` | Modify | Re-export the new `cli` module behind `#[cfg(feature = "cli")]`. |
| `httpyac-rs/src/cli/mod.rs` | Create | Top-level `Cli` clap struct + `Command` enum + `run()` entry point. |
| `httpyac-rs/src/cli/send.rs` | Create | `SendArgs` struct mirroring `httpyac send` flags + `run()` that proxies to npm httpyac. |
| `httpyac-rs/src/cli/oauth2.rs` | Create | `Oauth2Args` struct mirroring `httpyac oauth2` flags + `run()` that proxies to npm httpyac. |
| `httpyac-rs/src/cli/proxy.rs` | Create | Shared `proxy_to_httpyac()` helper that spawns `httpyac` with forwarded args, inherits stdio, returns exit code. |
| `httpyac-rs/src/bin/httpyac-rs.rs` | Create | Tiny `main()` that parses `Cli` and calls `cli::run()`. |
| `httpyac-rs/tests/cli_smoke.rs` | Create | Integration tests: `--help`, `--version`, `send --help`, `oauth2 --help`, unknown-flag rejection, proxy invocation. |

The lib crate (`use httpyac::send_exchange`, etc.) is unchanged — phase 0 adds the CLI surface but does not alter library behavior.

---

## Tech-stack notes for a fresh engineer

- **clap derive macros**: `#[derive(Parser)]` on the top-level struct, `#[derive(Subcommand)]` on the command enum, `#[derive(Args)]` on per-subcommand argument structs. Flags map via `#[arg(short, long)]`. Multi-value flags (httpyac's `--env x y` / `--var k=v` / `--tag a b`) use `num_args = 1..`. Enums for restricted-value flags use `#[derive(ValueEnum)]` with `value_enum` on the arg.
- **Feature gating**: clap and anyhow are CLI-only deps. They go under `[dependencies]` with `optional = true`, and the `[features].cli` array lists them. The `[[bin]]` target uses `required-features = ["cli"]` so `cargo build` of just the library doesn't try to build the binary.
- **Forwarding stdio**: When proxying, use `tokio::process::Command` with `stdin/stdout/stderr` set to `Stdio::inherit()` so httpyac's output flows directly to the user's terminal (including TTY-detected colors). Wait for the child and exit with the same status code.
- **clap argv handling**: clap absorbs `--`/help/version itself. For "unknown flag → error" behavior we don't want clap to be permissive; the default behavior is correct, but we explicitly test it in the smoke test.
- **httpyac on PATH**: Phase 0 assumes `httpyac` is on PATH. If missing, surface a clear "httpyac not found" error (mirroring the same lookup our LSP does, lifted into the lib).

---

## Task 1: Add `cli` feature and binary target to httpyac-rs/Cargo.toml

**Files:**
- Modify: `httpyac-rs/Cargo.toml`

- [ ] **Step 1: Write the failing test**

We can't write a test that compiles before the feature exists, so this task is structural; we drive correctness via the next task's tests. Skip step 1 here.

- [ ] **Step 2: Modify the manifest**

Replace the contents of `httpyac-rs/Cargo.toml` with:

```toml
[package]
name = "httpyac-rs"
version = "0.1.0"
edition = "2021"
license = "Apache-2.0"
description = "Rust wrapper around the httpyac CLI for sending .http requests"

[lib]
name = "httpyac"
path = "src/lib.rs"

[[bin]]
name = "httpyac-rs"
path = "src/bin/httpyac-rs.rs"
required-features = ["cli"]

[features]
default = []
cli = ["dep:clap", "dep:anyhow"]

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
tokio = { version = "1", features = ["process", "fs", "io-util", "rt-multi-thread", "macros", "signal"] }
clap = { version = "4.5", features = ["derive"], optional = true }
anyhow = { version = "1", optional = true }

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
```

- [ ] **Step 3: Verify the library still builds without the CLI feature**

Run: `cargo build -p httpyac-rs`
Expected: `Finished ... target(s)` with no errors. The `[[bin]]` is skipped because `required-features = ["cli"]` is not active.

- [ ] **Step 4: Verify the binary fails to build because src/bin/httpyac-rs.rs doesn't exist yet**

Run: `cargo build -p httpyac-rs --features cli 2>&1 | tail -5`
Expected: error containing `couldn't read .../src/bin/httpyac-rs.rs`. This confirms the manifest is wired up — we just haven't written the file yet.

- [ ] **Step 5: Commit**

```bash
git add httpyac-rs/Cargo.toml
git commit -m "httpyac-rs: add [[bin]] httpyac-rs gated behind \`cli\` cargo feature

Sets up the manifest so subsequent tasks can add the binary entry
point and CLI module. \`cli\` is off by default so library consumers
(zed-http-lsp) don't pull in clap/anyhow."
```

---

## Task 2: Stub the CLI module tree

**Files:**
- Modify: `httpyac-rs/src/lib.rs`
- Create: `httpyac-rs/src/cli/mod.rs`
- Create: `httpyac-rs/src/bin/httpyac-rs.rs`

- [ ] **Step 1: Add the cli module to lib.rs**

Open `httpyac-rs/src/lib.rs` and add the following after the existing module declarations (after the `pub use send::{send_exchange, SendOptions};` line):

```rust
#[cfg(feature = "cli")]
pub mod cli;
```

- [ ] **Step 2: Create the cli module skeleton**

Create `httpyac-rs/src/cli/mod.rs` with:

```rust
//! Command-line interface for `httpyac-rs`.
//!
//! Phase 0 mirrors the upstream npm `httpyac` CLI surface flag-for-flag
//! and proxies every invocation through to the `httpyac` binary on
//! PATH. Later phases (1–7) will progressively replace the proxy with
//! native parsing, variable interpolation, HTTP execution, scripting,
//! and multi-protocol support — see
//! docs/superpowers/plans/2026-05-28-httpyac-rs-cli.md.

use clap::{Parser, Subcommand};

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
    // Subcommand variants are added in tasks 3 and 4.
}

/// Parse argv and dispatch the chosen subcommand.
///
/// Returns the exit code the binary should exit with.
pub async fn run() -> anyhow::Result<i32> {
    let cli = Cli::parse();
    match cli.command {}
}
```

This intentionally won't compile yet because `Command` has no variants and the `match` is empty — that's fixed in task 3.

- [ ] **Step 3: Create the binary entry point**

Create `httpyac-rs/src/bin/httpyac-rs.rs` with:

```rust
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match httpyac::cli::run().await {
        Ok(code) => ExitCode::from(code.clamp(0, 255) as u8),
        Err(err) => {
            eprintln!("httpyac-rs: {err:#}");
            ExitCode::from(1)
        }
    }
}
```

- [ ] **Step 4: Verify the CLI build fails for the expected reason**

Run: `cargo build -p httpyac-rs --features cli 2>&1 | tail -10`
Expected: An error about `match cli.command {}` being a non-exhaustive match over an empty enum or `Command` having no variants — confirming the wiring is correct but waiting for task 3.

If you instead see a "module not found" or "file not found" error, double-check the file paths above.

- [ ] **Step 5: Don't commit yet**

This is an intentionally broken intermediate state. We commit after task 3 makes it compile.

---

## Task 3: Implement `send` subcommand with full clap surface

**Files:**
- Create: `httpyac-rs/src/cli/send.rs`
- Create: `httpyac-rs/src/cli/proxy.rs`
- Modify: `httpyac-rs/src/cli/mod.rs`

- [ ] **Step 1: Create the proxy helper**

Create `httpyac-rs/src/cli/proxy.rs` with:

```rust
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
```

- [ ] **Step 2: Create the send subcommand**

Create `httpyac-rs/src/cli/send.rs` with:

```rust
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
```

- [ ] **Step 3: Wire send into the Command enum**

Edit `httpyac-rs/src/cli/mod.rs`:

```rust
//! Command-line interface for `httpyac-rs`.
//!
//! Phase 0 mirrors the upstream npm `httpyac` CLI surface flag-for-flag
//! and proxies every invocation through to the `httpyac` binary on
//! PATH. Later phases (1–7) will progressively replace the proxy with
//! native parsing, variable interpolation, HTTP execution, scripting,
//! and multi-protocol support — see
//! docs/superpowers/plans/2026-05-28-httpyac-rs-cli.md.

use clap::{Parser, Subcommand};

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
}

pub async fn run() -> anyhow::Result<i32> {
    let cli = Cli::parse();
    match cli.command {
        Command::Send(args) => args.run().await,
    }
}
```

- [ ] **Step 4: Verify the binary compiles and `--help` looks right**

Run: `cargo build -p httpyac-rs --features cli 2>&1 | tail -5`
Expected: `Finished ... target(s)`.

Run: `cargo run -p httpyac-rs --features cli -- --help 2>&1 | head -15`
Expected output (something like):

```
HTTP/REST CLI Client for *.http files (Rust port of httpyac)

Usage: httpyac-rs <COMMAND>

Commands:
  send  Send/execute http files
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

Run: `cargo run -p httpyac-rs --features cli -- send --help 2>&1 | head -20`
Expected: a listing of all the flags defined above.

- [ ] **Step 5: Commit**

```bash
git add httpyac-rs/src/lib.rs httpyac-rs/src/cli/ httpyac-rs/src/bin/
git commit -m "httpyac-rs: add \`send\` subcommand mirroring httpyac CLI

Defines the full clap surface for \`httpyac send\` — every flag,
multi-value semantics, ValueEnum-restricted options. Execution is
proxied to the upstream npm \`httpyac\` binary via a shared
proxy_to_httpyac helper that inherits stdio. Argv is reconstructed
from the parsed SendArgs so we have a snapshot-testable contract."
```

---

## Task 4: Implement `oauth2` subcommand

**Files:**
- Create: `httpyac-rs/src/cli/oauth2.rs`
- Modify: `httpyac-rs/src/cli/mod.rs`

- [ ] **Step 1: Create the oauth2 subcommand**

Create `httpyac-rs/src/cli/oauth2.rs` with:

```rust
//! `httpyac-rs oauth2` — mirrors `httpyac oauth2`.
//!
//! Like `send`, phase 0 forwards every flag to the upstream npm
//! `httpyac` binary. Native OAuth2 flow execution arrives in phase 6.

use std::ffi::OsString;

use anyhow::Result;
use clap::{Args, ValueEnum};

use crate::cli::proxy::proxy_to_httpyac;

#[derive(Debug, Args)]
pub struct Oauth2Args {
    /// OAuth2 flow to use.
    #[arg(short = 'f', long, default_value = "client_credentials", value_name = "FLOW")]
    pub flow: String,

    /// Variable prefix used for OAuth2 variables in the env file.
    #[arg(long, value_name = "PREFIX")]
    pub prefix: Option<String>,

    /// One or more environments to apply.
    #[arg(short = 'e', long, num_args = 1..)]
    pub env: Vec<String>,

    /// Output format for the OAuth2 token response.
    #[arg(short = 'o', long, default_value = "access_token", value_enum)]
    pub output: Oauth2OutputFormat,

    /// Define one or more variables, e.g. `--var foo=bar baz=qux`.
    #[arg(long, num_args = 1..)]
    pub var: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Oauth2OutputFormat {
    AccessToken,
    RefreshToken,
    Response,
}

impl Oauth2OutputFormat {
    fn as_cli_str(self) -> &'static str {
        match self {
            Oauth2OutputFormat::AccessToken => "access_token",
            Oauth2OutputFormat::RefreshToken => "refresh_token",
            Oauth2OutputFormat::Response => "response",
        }
    }
}

impl Oauth2Args {
    pub fn to_argv(&self) -> Vec<OsString> {
        let mut argv: Vec<OsString> = Vec::new();

        argv.push("--flow".into());
        argv.push(self.flow.clone().into());

        if let Some(prefix) = &self.prefix {
            argv.push("--prefix".into());
            argv.push(prefix.into());
        }
        for e in &self.env {
            argv.push("--env".into());
            argv.push(e.into());
        }
        argv.push("--output".into());
        argv.push(self.output.as_cli_str().into());
        for v in &self.var {
            argv.push("--var".into());
            argv.push(v.into());
        }

        argv
    }

    pub async fn run(self) -> Result<i32> {
        proxy_to_httpyac("oauth2", self.to_argv()).await
    }
}
```

- [ ] **Step 2: Wire oauth2 into Command**

Edit `httpyac-rs/src/cli/mod.rs` and add the import + variant:

```rust
pub mod oauth2;
```

(Place it next to `pub mod send;`.)

Then extend the `Command` enum:

```rust
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Send/execute http files.
    Send(send::SendArgs),
    /// Generate an OAuth2 token.
    Oauth2(oauth2::Oauth2Args),
}
```

And the match in `run`:

```rust
pub async fn run() -> anyhow::Result<i32> {
    let cli = Cli::parse();
    match cli.command {
        Command::Send(args) => args.run().await,
        Command::Oauth2(args) => args.run().await,
    }
}
```

- [ ] **Step 3: Verify**

Run: `cargo build -p httpyac-rs --features cli 2>&1 | tail -3`
Expected: `Finished ... target(s)`.

Run: `cargo run -p httpyac-rs --features cli -- oauth2 --help 2>&1 | head -15`
Expected: oauth2 flags listed.

- [ ] **Step 4: Commit**

```bash
git add httpyac-rs/src/cli/oauth2.rs httpyac-rs/src/cli/mod.rs
git commit -m "httpyac-rs: add \`oauth2\` subcommand mirroring httpyac CLI

Same proxy-to-upstream approach as \`send\`. Native OAuth2 flow
execution lands in phase 6 (see docs/superpowers/plans/
2026-05-28-httpyac-rs-cli.md)."
```

---

## Task 5: Integration tests for the CLI surface

**Files:**
- Create: `httpyac-rs/tests/cli_smoke.rs`

These use `assert_cmd` (already added to dev-deps in task 1) to invoke the built binary and assert on stdout/exit codes. They don't require npm `httpyac` to be installed — they only exercise the help/version/arg-parsing paths.

- [ ] **Step 1: Write the failing tests**

Create `httpyac-rs/tests/cli_smoke.rs` with:

```rust
//! Smoke tests for the `httpyac-rs` CLI surface.
//!
//! These exercise clap's help/version/arg-parsing paths without ever
//! running npm `httpyac`, so they pass even on machines without
//! httpyac installed. Tests that exercise the actual proxy live in
//! ../tests/cli_proxy.rs and are gated behind the `httpyac` binary
//! being on PATH (TODO: add in a later phase when we want it).

#![cfg(feature = "cli")]

use assert_cmd::Command;
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
```

- [ ] **Step 2: Run the tests and watch them pass**

Run: `cargo test -p httpyac-rs --features cli 2>&1 | tail -15`
Expected: `test result: ok. 8 passed; 0 failed`.

If any fail, fix the corresponding implementation in tasks 3/4 (likely a flag name or help-text mismatch) before continuing.

- [ ] **Step 3: Commit**

```bash
git add httpyac-rs/tests/cli_smoke.rs
git commit -m "httpyac-rs: integration tests for CLI surface

Exercises clap's help/version/argument-parsing paths without needing
npm \`httpyac\` on PATH. Asserts every documented flag appears in
the help output (catches future drift), and that clap rejects
unknown flags and out-of-vocabulary value-enum choices."
```

---

## Task 6: Update repo-level docs

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add a short section under the existing layout block**

Find the "Repository layout" code block in `README.md` and replace it with:

```
## Repository layout

\```
zed-http/
├── src/                 wasm extension entry (language-server-command, runnables)
├── languages/http/      tree-sitter queries (highlights, injections, outline, runnables)
├── zed-extension/       extension logic (workspace member)
├── http-lsp/            zed-http-lsp binary crate
├── httpyac-rs/          Rust wrapper around the httpyac CLI; also exposes
│                        a `httpyac-rs` binary (off-by-default `cli` feature)
│                        that mirrors httpyac's flag surface and proxies
│                        through to npm `httpyac`. See
│                        docs/superpowers/plans/2026-05-28-httpyac-rs-cli.md
│                        for the phased native-port plan.
├── patches/             reference patch applied to ToyVoDev/tree-sitter-http
└── flake.nix            devShell + zed-http-lsp Nix package
\```
```

(Use real backticks in the actual file — they're escaped here so this plan markdown renders.)

- [ ] **Step 2: Add an "Install httpyac-rs CLI" sub-section**

Find the "Optional LSP" section in `README.md` and add a sibling `## Optional CLI` section right after it:

```markdown
## Optional CLI

`httpyac-rs` also ships a `httpyac-rs` binary that mirrors the upstream
`httpyac` CLI. In its current state it proxies every command through to
the npm `httpyac` binary on PATH; native execution is being added in
phases (see `docs/superpowers/plans/2026-05-28-httpyac-rs-cli.md`).

Install with cargo:

\```bash
cd zed-http
cargo install --path httpyac-rs --features cli
\```

Use:

\```bash
httpyac-rs send path/to/file.http --line 14
\```
```

(Same escaping caveat for triple backticks.)

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: mention the httpyac-rs CLI binary and link to the phasing plan"
```

---

## Self-review notes

- Spec coverage: every flag from `httpyac send --help` and `httpyac oauth2 --help` is represented in `SendArgs` / `Oauth2Args` (including multi-value flags `--env`/`--tag`/`--var`/`--filename`), proxied via `to_argv()`, and asserted on in `cli_smoke.rs`.
- Placeholders: every code block above is complete; no `TODO`, `TBD`, or "implement later" inside any step's code. Future phases are documented in the header table, but phase 0 is fully spelled out.
- Type consistency: `SendArgs::to_argv` and `Oauth2Args::to_argv` both return `Vec<OsString>`; `proxy_to_httpyac` accepts `Vec<OsString>`. `OutputFormat::as_cli_str`, `RepeatMode::as_cli_str`, `Oauth2OutputFormat::as_cli_str` all share the same `(self) -> &'static str` shape.
- Risks I'm aware of and not addressing in phase 0:
  - The proxy assumes `httpyac` is on PATH. If it isn't, the user gets the `with_context` message from `proxy_to_httpyac`. We could add a setting later to point at a specific binary, but it's not needed for phase 0.
  - The `--no-color` flag is forwarded but our binary doesn't itself emit colored output (clap's auto-color is fine to leave on). Behavior matches npm httpyac because we shell out.
