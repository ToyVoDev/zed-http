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

#[derive(Clone)]
pub struct Backend {
    // Every field here is cheap-clone (Arc / DashMap-via-Arc) so the whole
    // backend can be cloned into a tokio::spawn for deferred work without
    // moving any owned state.
    client: Client,
    requests: Arc<DashMap<Url, Vec<Request>>>,
    cache: Arc<ResponseCache>,
    config: Arc<tokio::sync::RwLock<Config>>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            requests: Arc::new(DashMap::new()),
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

    fn enclosing_request(&self, uri: &Url, line: u32) -> Option<Request> {
        let reqs = self.requests.get(uri)?;
        request_index::request_at_line(&reqs, line).cloned()
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
                // CodeLens is what Zed routes through workspace/executeCommand for
                // clickable inline commands; inlay hint `command` fields are silently
                // dropped by Zed's lsp_inlay_label_to_project conversion.
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
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
        self.reindex(&params.text_document.uri, &params.text_document.text);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            self.reindex(&uri, &change.text);
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.requests.remove(&params.text_document.uri);
    }

    async fn code_lens(&self, params: CodeLensParams) -> LspResult<Option<Vec<CodeLens>>> {
        let uri = params.text_document.uri;
        let Some(reqs) = self.requests.get(&uri) else {
            return Ok(None);
        };

        let mut lenses = Vec::with_capacity(reqs.len() * 4);
        for r in reqs.iter() {
            let range = Range {
                start: Position { line: r.line, character: 0 },
                end: Position { line: r.line, character: 0 },
            };
            let has_cached = self.cache.get(&uri, r.line).is_some();

            lenses.push(lens(range, "▶ Send", CMD_SEND, &uri, r.line));
            if has_cached {
                lenses.push(lens(range, "👁 Show", CMD_SHOW, &uri, r.line));
                lenses.push(lens(range, "◉ Headers", CMD_HEADERS, &uri, r.line));
            }
            lenses.push(lens(range, "💾 Save", CMD_SAVE, &uri, r.line));
        }
        Ok(Some(lenses))
    }

    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> LspResult<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let line = params.range.start.line;
        let Some(req) = self.enclosing_request(&uri, line) else {
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
        let Some(req) = self.enclosing_request(&uri, line) else {
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

        // Defer the work to a background task and return Ok(None) immediately.
        //
        // When apply_code_action / apply_code_lens in Zed sees a `workspace/
        // applyEdit` come back during the executeCommand request, it both
        // (a) emits WorkspaceEditApplied which the editor handles by opening
        // the new buffer, and (b) stores the same transaction in
        // last_workspace_edits_by_language_server which apply_code_action
        // returns to the caller — which then calls open_project_transaction
        // with the code action's title. Two opens for the same buffer ("LSP
        // Edit" plus the action title).
        //
        // By spawning the actual work, executeCommand returns an empty
        // transaction, so apply_code_action's open is a no-op, and only the
        // event-driven open happens.
        let backend = self.clone();
        let command = params.command;
        tokio::spawn(async move {
            match command.as_str() {
                CMD_SEND => backend.run_send(uri, line).await,
                CMD_SHOW => backend.run_show(uri, line, View::Full).await,
                CMD_HEADERS => backend.run_show(uri, line, View::HeadersOnly).await,
                CMD_SAVE => backend.run_save(uri, line).await,
                other => {
                    backend
                        .client
                        .show_message(
                            MessageType::ERROR,
                            format!("zed-http: unknown command {other}"),
                        )
                        .await;
                }
            }
        });

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

        let Ok(target_uri) = Url::from_file_path(&target) else {
            return;
        };
        if let Err(e) = self.write_and_open(target_uri, &body).await {
            self.client
                .show_message(MessageType::ERROR, format!("zed-http: save failed: {e}"))
                .await;
        }
    }

    async fn open_response(&self, uri: &Url, line: u32, exchange: &Exchange, view: View) {
        let body = response_format::format(exchange, view);
        let path = match temp_response_path(uri, line).await {
            Ok(p) => p,
            Err(e) => {
                self.client
                    .show_message(
                        MessageType::ERROR,
                        format!("zed-http: failed to allocate response temp file: {e}"),
                    )
                    .await;
                return;
            }
        };
        self.cache.attach_temp_path(uri, line, path.clone());

        let Ok(target_uri) = Url::from_file_path(&path) else {
            return;
        };
        if let Err(e) = self.write_and_open(target_uri, &body).await {
            self.client
                .show_message(
                    MessageType::ERROR,
                    format!("zed-http: failed to display response: {e}"),
                )
                .await;
        }
    }

    /// Create the file at `target_uri` and populate it with `body` via a single
    /// `workspace/applyEdit` — Zed's editor reacts to the resulting
    /// `WorkspaceEditApplied` event by opening hidden buffers, so the new
    /// response file shows up in a fresh pane without us needing
    /// `window/showDocument` (which Zed doesn't implement).
    async fn write_and_open(&self, target_uri: Url, body: &str) -> Result<(), String> {
        if let Some(parent) = target_uri.to_file_path().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())) {
            if let Err(e) = tokio::fs::create_dir_all(&parent).await {
                return Err(e.to_string());
            }
        }

        let edit = WorkspaceEdit {
            document_changes: Some(DocumentChanges::Operations(vec![
                DocumentChangeOperation::Op(ResourceOp::Create(CreateFile {
                    uri: target_uri.clone(),
                    options: Some(CreateFileOptions {
                        overwrite: Some(true),
                        ignore_if_exists: None,
                    }),
                    annotation_id: None,
                })),
                DocumentChangeOperation::Edit(TextDocumentEdit {
                    text_document: OptionalVersionedTextDocumentIdentifier {
                        uri: target_uri,
                        version: None,
                    },
                    edits: vec![OneOf::Left(TextEdit {
                        range: Range {
                            start: Position { line: 0, character: 0 },
                            end: Position { line: 0, character: 0 },
                        },
                        new_text: body.to_string(),
                    })],
                }),
            ])),
            changes: None,
            change_annotations: None,
        };

        match self.client.apply_edit(edit).await {
            Ok(resp) if resp.applied => Ok(()),
            Ok(resp) => Err(resp
                .failure_reason
                .unwrap_or_else(|| "client refused workspace edit".into())),
            Err(e) => Err(e.to_string()),
        }
    }
}

fn lens(range: Range, title: &str, command: &str, uri: &Url, line: u32) -> CodeLens {
    CodeLens {
        range,
        command: Some(Command {
            title: title.into(),
            command: command.into(),
            arguments: Some(vec![Value::String(uri.to_string()), Value::from(line)]),
        }),
        data: None,
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

async fn temp_response_path(uri: &Url, line: u32) -> std::io::Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("zed-http")
        .join("responses");
    tokio::fs::create_dir_all(&cache_dir).await?;

    let base = uri_basename(uri).unwrap_or_else(|| "response".into());
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    Ok(cache_dir.join(format!("{base}-line{line}-{ts}.http-resp")))
}

