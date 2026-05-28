use std::path::Path;
use std::process::Stdio;

use tokio::process::Command;

use crate::{Error, Exchange};

/// Options for [`send_exchange`].
///
/// `file` must point at a real file on disk that's inside `file.parent()` —
/// httpyac requires the file argument to be inside its cwd.
pub struct SendOptions<'a> {
    /// Name or path of the httpyac binary. Usually just `"httpyac"`.
    pub binary: &'a str,
    /// Absolute path to the `.http` file to send.
    pub file: &'a Path,
    /// 0-indexed line of the request to execute (will be translated to
    /// httpyac's 1-indexed `--line` flag).
    pub line: u32,
}

/// Invoke `httpyac send <file> --line <N+1> --json --output exchange` and parse
/// the resulting JSON into an [`Exchange`].
pub async fn send_exchange(opts: SendOptions<'_>) -> Result<Exchange, Error> {
    let parent = opts
        .file
        .parent()
        .ok_or_else(|| Error::NoParent(opts.file.to_path_buf()))?;
    let file_name = opts
        .file
        .file_name()
        .ok_or_else(|| Error::NoParent(opts.file.to_path_buf()))?;

    let line_arg = (opts.line + 1).to_string();

    let mut cmd = Command::new(opts.binary);
    cmd.current_dir(parent)
        .arg("send")
        .arg(file_name)
        .arg("--line")
        .arg(&line_arg)
        .arg("--json")
        .arg("--output")
        .arg("exchange")
        .arg("--no-color")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = match cmd.output().await {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(Error::NotFound {
                binary: opts.binary.to_string(),
            });
        }
        Err(e) => return Err(Error::Io(e)),
    };

    if !output.status.success() {
        return Err(Error::NonZero {
            status: output.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let exchange: Exchange = serde_json::from_slice(&output.stdout)?;
    Ok(exchange)
}
