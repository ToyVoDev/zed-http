//! Rust wrapper around the [`httpyac`](https://httpyac.github.io/) CLI.
//!
//! The `httpyac` binary handles all the heavy lifting (variable interpolation,
//! scripting, multi-protocol support, environment files, etc.). This crate
//! just spawns it with the right arguments, sets cwd correctly, and parses the
//! `--json --output exchange` payload into typed Rust structs.
//!
//! ```no_run
//! use httpyac::{send_exchange, SendOptions};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let exchange = send_exchange(SendOptions {
//!     binary: "httpyac",
//!     file: std::path::Path::new("/path/to/requests.http"),
//!     line: 0,
//! })
//! .await?;
//!
//! if let Some(resp) = exchange.requests.first().and_then(|r| r.response.as_ref()) {
//!     println!("{} {}", resp.status_code, resp.status_message.as_deref().unwrap_or(""));
//! }
//! # Ok(())
//! # }
//! ```

mod error;
mod exchange;
mod send;

pub use error::Error;
pub use exchange::{Exchange, Meta, RequestResult, Response, Summary, Timings};
pub use send::{send_exchange, SendOptions};

#[cfg(feature = "cli")]
pub mod cli;
