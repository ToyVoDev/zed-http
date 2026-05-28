use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use serde::Deserialize;
use serde_json::Value;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use httpyac::{Exchange, SendOptions};

use crate::cache::ResponseCache;
use crate::request_index::{self, Request};
use crate::response_format::{self, View};

const CMD_SEND: &str = "zed-http.send";
const CMD_SHOW: &str = "zed-http.show";
const CMD_SAVE: &str = "zed-http.save";
const CMD_HEADERS: &str = "zed-http.headers";
const ALL_COMMANDS: &[&str] = &[CMD_SEND, CMD_SHOW, CMD_SAVE, CMD_HEADERS];

#[derive(Debug, Default)]
struct Config {
    /// Path/name of the httpyac executable. Defaults to "httpyac" (PATH lookup).
    httpyac_path: Option<String>,
}

impl Config {
    fn binary(&self) -> &str {
        self.httpyac_path.as_deref().unwrap_or("httpyac")
    }
}

#[derive(Debug, Deserialize, Default)]
struct UserSettings {
    #[serde(default)]
    httpyac: Option<HttpYacSettings>,
}

#[derive(Debug, Deserialize, Default)]
struct HttpYacSettings {
    #[serde(default)]
    path: Option<String>,
}

pub struct Backend {
    client: Client,
    documents: DashMap<Url, String>,
    requests: DashMap<Url, Vec<Request>>,
    cache: Arc<ResponseCache>,
    config: Arc<tokio::sync::RwLock<Config>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            documents: DashMap::new(),
            requests: DashMap::new(),
            cache: Arc::new(ResponseCache::new()),
            config: Arc::new(tokio::sync::RwLock::new(Config::default())),
        }
    }

    fn reindex(&self, uri: &Url, text: &str) {
        let reqs = request_index::scan(text);
        self.requests.insert(uri.clone(), reqs);
    }

    async fn binary(&self) -> String {
        self.config.read().await.binary().to_string()
    }

    fn enclosing_request(&self, uri: &Url, line: u32) -> Option<(Request, u32)> {
        let reqs = self.requests.get(uri)?;
        let text = self.documents.get(uri)?;
        let total = text.lines().count() as u32;
        let r = request_index::request_at_line(&reqs, line, total)?.clone();
        Some((r, total))
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _params: InitializeParams) -> LspResult<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: "zed-http-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                inlay_hint_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![
                            CodeActionKind::new("zed-http"),
                            CodeActionKind::EMPTY,
                        ]),
                        resolve_provider: Some(false),
                        work_done_progress_options: Default::default(),
                    },
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: ALL_COMMANDS.iter().map(|s| s.to_string()).collect(),
                    work_done_progress_options: Default::default(),
                }),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "zed-http-lsp ready")
            .await;
    }

    async fn did_change_configuration(&self, params: DidChangeConfigurationParams) {
        if let Ok(settings) = serde_json::from_value::<UserSettings>(params.settings) {
            let mut cfg = self.config.write().await;
            cfg.httpyac_path = settings.httpyac.and_then(|h| h.path);
        }
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.reindex(&uri, &text);
        self.documents.insert(uri, text);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            self.reindex(&uri, &change.text);
            self.documents.insert(uri, change.text);
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = &params.text_document.uri;
        self.documents.remove(uri);
        self.requests.remove(uri);
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> LspResult<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        let Some(reqs) = self.requests.get(&uri) else {
            return Ok(None);
        };

        let mut hints = Vec::with_capacity(reqs.len() * 4);
        for r in reqs.iter() {
            let position = Position {
                line: r.line,
                character: u32::MAX,
            };
            let has_cached = self.cache.get(&uri, r.line).is_some();
            hints.push(InlayHint {
                position,
                label: InlayHintLabel::LabelParts(label_parts(&uri, r.line, has_cached)),
                kind: None,
                text_edits: None,
                tooltip: Some(InlayHintTooltip::String(format!("{} {}", r.method, r.url))),
                padding_left: Some(true),
                padding_right: None,
                data: None,
            });
        }
        Ok(Some(hints))
    }

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> LspResult<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let line = params.range.start.line;
        let Some((req, _total)) = self.enclosing_request(&uri, line) else {
            return Ok(None);
        };

        let has_cached = self.cache.get(&uri, req.line).is_some();
        let mut actions: Vec<CodeActionOrCommand> =
            Vec::with_capacity(if has_cached { 4 } else { 2 });

        actions.push(action(
            "▶ Send Request",
            CMD_SEND,
            &uri,
            req.line,
        ));
        if has_cached {
            actions.push(action("👁 Show Response", CMD_SHOW, &uri, req.line));
            actions.push(action("◉ Show Headers", CMD_HEADERS, &uri, req.line));
        }
        actions.push(action("💾 Save Response", CMD_SAVE, &uri, req.line));

        Ok(Some(actions))
    }

    async fn hover(&self, params: HoverParams) -> LspResult<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let line = params.text_document_position_params.position.line;
        let Some((req, _)) = self.enclosing_request(&uri, line) else {
            return Ok(None);
        };

        let mut md = format!("**{}** `{}`\n", req.method, req.url);
        if let Some(cached) = self.cache.get(&uri, req.line) {
            if let Some(resp) = cached.exchange.requests.first().and_then(|r| r.response.as_ref()) {
                let dur = resp
                    .timings
                    .as_ref()
                    .and_then(|t| t.total)
                    .map(|d| format!(" · {d:.0} ms"))
                    .unwrap_or_default();
                let when = cached.at.format("%H:%M:%S");
                md.push_str(&format!(
                    "\nLast response: **{} {}**{} at {when}\n",
                    resp.status_code,
                    resp.status_message.as_deref().unwrap_or(""),
                    dur,
                ));
            }
        } else {
            md.push_str("\n_No response cached. Trigger ‘Send Request’ from a code action._\n");
        }

        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: md,
            }),
            range: None,
        }))
    }

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> LspResult<Option<Value>> {
        let Some((uri, line)) = parse_args(&params.arguments) else {
            self.client
                .show_message(MessageType::ERROR, "zed-http: missing uri/line arguments")
                .await;
            return Ok(None);
        };

        match params.command.as_str() {
            CMD_SEND => {
                self.run_send(uri, line).await;
            }
            CMD_SHOW => {
                self.run_show(uri, line, View::Full).await;
            }
            CMD_HEADERS => {
                self.run_show(uri, line, View::HeadersOnly).await;
            }
            CMD_SAVE => {
                self.run_save(uri, line).await;
            }
            other => {
                self.client
                    .show_message(
                        MessageType::ERROR,
                        format!("zed-http: unknown command {other}"),
                    )
                    .await;
            }
        }
        Ok(None)
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }
}

impl Backend {
    async fn run_send(&self, uri: Url, line: u32) {
        let Some(path) = uri_to_path(&uri) else {
            self.client
                .show_message(MessageType::ERROR, format!("zed-http: non-file URI {uri}"))
                .await;
            return;
        };
        let binary = self.binary().await;
        let exchange = match httpyac::send_exchange(SendOptions {
            binary: &binary,
            file: &path,
            line,
        })
        .await
        {
            Ok(e) => e,
            Err(e) => {
                self.client
                    .show_message(MessageType::ERROR, format!("zed-http: {e}"))
                    .await;
                return;
            }
        };

        self.cache.insert(uri.clone(), line, exchange.clone());
        self.open_response(&uri, line, &exchange, View::Full).await;
    }

    async fn run_show(&self, uri: Url, line: u32, view: View) {
        let Some(cached) = self.cache.get(&uri, line) else {
            self.client
                .show_message(
                    MessageType::WARNING,
                    "zed-http: no cached response for this request — send it first",
                )
                .await;
            return;
        };
        self.open_response(&uri, line, &cached.exchange, view).await;
    }

    async fn run_save(&self, uri: Url, line: u32) {
        let Some(cached) = self.cache.get(&uri, line) else {
            self.client
                .show_message(
                    MessageType::WARNING,
                    "zed-http: no cached response to save — send the request first",
                )
                .await;
            return;
        };

        let Some(parent) = uri_to_path(&uri).and_then(|p| p.parent().map(|p| p.to_path_buf())) else {
            return;
        };

        let base = uri_basename(&uri).unwrap_or_else(|| "response".into());
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let target = parent.join(format!("{base}-line{line}-{ts}.resp"));
        let body = response_format::format(&cached.exchange, View::Full);

        if let Err(e) = tokio::fs::write(&target, body).await {
            self.client
                .show_message(MessageType::ERROR, format!("zed-http: save failed: {e}"))
                .await;
            return;
        }

        if let Ok(target_uri) = Url::from_file_path(&target) {
            let _ = self
                .client
                .show_document(ShowDocumentParams {
                    uri: target_uri,
                    external: Some(false),
                    take_focus: Some(true),
                    selection: None,
                })
                .await;
        }
    }

    async fn open_response(&self, uri: &Url, line: u32, exchange: &Exchange, view: View) {
        let body = response_format::format(exchange, view);
        let path = match write_temp(uri, line, &body).await {
            Ok(p) => p,
            Err(e) => {
                self.client
                    .show_message(
                        MessageType::ERROR,
                        format!("zed-http: failed to write response temp file: {e}"),
                    )
                    .await;
                return;
            }
        };
        self.cache.attach_temp_path(uri, line, path.clone());

        if let Ok(target_uri) = Url::from_file_path(&path) {
            let _ = self
                .client
                .show_document(ShowDocumentParams {
                    uri: target_uri,
                    external: Some(false),
                    take_focus: Some(true),
                    selection: None,
                })
                .await;
        }
    }
}

fn label_parts(uri: &Url, line: u32, has_cached: bool) -> Vec<InlayHintLabelPart> {
    let mut parts = vec![
        sep(),
        labeled_part(
            "▶ Send",
            "Send this HTTP request via httpyac",
            CMD_SEND,
            uri,
            line,
        ),
    ];
    if has_cached {
        parts.push(sep());
        parts.push(labeled_part(
            "👁 Show",
            "Re-open the last response for this request",
            CMD_SHOW,
            uri,
            line,
        ));
        parts.push(sep());
        parts.push(labeled_part(
            "◉ Headers",
            "Show only response headers from the last response",
            CMD_HEADERS,
            uri,
            line,
        ));
    }
    parts.push(sep());
    parts.push(labeled_part(
        "💾 Save",
        "Save the last response to a file next to this .http file",
        CMD_SAVE,
        uri,
        line,
    ));
    parts
}

fn sep() -> InlayHintLabelPart {
    InlayHintLabelPart {
        value: " ".into(),
        tooltip: None,
        location: None,
        command: None,
    }
}

fn labeled_part(
    label: &str,
    tooltip: &str,
    command: &str,
    uri: &Url,
    line: u32,
) -> InlayHintLabelPart {
    InlayHintLabelPart {
        value: label.into(),
        tooltip: Some(InlayHintLabelPartTooltip::String(tooltip.into())),
        location: None,
        command: Some(Command {
            title: label.into(),
            command: command.into(),
            arguments: Some(vec![Value::String(uri.to_string()), Value::from(line)]),
        }),
    }
}

fn action(title: &str, command: &str, uri: &Url, line: u32) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: title.into(),
        kind: Some(CodeActionKind::new("zed-http")),
        diagnostics: None,
        edit: None,
        command: Some(Command {
            title: title.into(),
            command: command.into(),
            arguments: Some(vec![Value::String(uri.to_string()), Value::from(line)]),
        }),
        is_preferred: None,
        disabled: None,
        data: None,
    })
}

fn parse_args(args: &[Value]) -> Option<(Url, u32)> {
    let uri = args.first()?.as_str()?;
    let line = args.get(1)?.as_u64()? as u32;
    Url::parse(uri).ok().map(|u| (u, line))
}

fn uri_to_path(uri: &Url) -> Option<PathBuf> {
    uri.to_file_path().ok()
}

fn uri_basename(uri: &Url) -> Option<String> {
    let path = uri_to_path(uri)?;
    path.file_stem().map(|s| s.to_string_lossy().into_owned())
}

async fn write_temp(uri: &Url, line: u32, body: &str) -> std::io::Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("zed-http")
        .join("responses");
    tokio::fs::create_dir_all(&cache_dir).await?;

    let base = uri_basename(uri).unwrap_or_else(|| "response".into());
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = cache_dir.join(format!("{base}-line{line}-{ts}.http-resp"));
    tokio::fs::write(&path, body).await?;
    Ok(path)
}

