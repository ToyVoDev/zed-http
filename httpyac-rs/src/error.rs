use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("httpyac binary not found at `{binary}` — install httpyac or point at an explicit path")]
    NotFound { binary: String },

    #[error("httpyac exited with status {status}: {stderr}")]
    NonZero { status: i32, stderr: String },

    #[error("httpyac output is not valid JSON: {0}")]
    BadJson(#[from] serde_json::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("file path has no parent directory: {0}")]
    NoParent(PathBuf),
}
