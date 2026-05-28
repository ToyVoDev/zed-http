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
    #[value(name = "access_token")]
    AccessToken,
    #[value(name = "refresh_token")]
    RefreshToken,
    #[value(name = "response")]
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
