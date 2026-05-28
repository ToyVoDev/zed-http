use zed_extension_api::{self as zed, settings::LspSettings};

const LSP_BINARY: &str = "zed-http-lsp";

pub struct HttpExtension {}

impl zed::Extension for HttpExtension {
    fn new() -> Self {
        Self {}
    }

    fn language_server_command(
        &mut self,
        config: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        // Resolve binary path: user setting -> PATH lookup -> common cargo/local bins.
        let configured = LspSettings::for_worktree(config.as_ref(), worktree)
            .ok()
            .and_then(|s| s.binary)
            .and_then(|b| b.path);

        let command = configured
            .or_else(|| worktree.which(LSP_BINARY))
            .or_else(|| fallback_path())
            .ok_or_else(|| {
                format!(
                    "could not find `{LSP_BINARY}` on PATH. Install with \
                     `cargo install --path http-lsp` from the zed-http checkout, \
                     or set `lsp.zed-http-lsp.binary.path` in your Zed settings."
                )
            })?;

        Ok(zed::Command {
            command,
            args: Vec::new(),
            env: worktree.shell_env(),
        })
    }
}

fn fallback_path() -> Option<String> {
    let candidates = [
        env_home().map(|h| format!("{h}/.cargo/bin/{LSP_BINARY}")),
        env_home().map(|h| format!("{h}/.local/bin/{LSP_BINARY}")),
        Some(format!("/usr/local/bin/{LSP_BINARY}")),
    ];
    candidates.into_iter().flatten().find(|p| std::path::Path::new(p).exists())
}

fn env_home() -> Option<String> {
    std::env::var("HOME").ok()
}
