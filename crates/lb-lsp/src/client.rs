use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Initialized,
};
use lsp_types::request::{
    Completion, GotoDefinition, HoverRequest, Initialize, Shutdown,
};
use lsp_types::{
    ClientCapabilities, CompletionContext, CompletionParams, CompletionResponse,
    CompletionTriggerKind, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams,
    InitializeParams, InitializedParams, Position, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams, Uri,
    VersionedTextDocumentIdentifier, WorkspaceClientCapabilities,
};
use serde_json::Value;
use tokio::io::BufReader;
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, warn};

use crate::transport::{read_message, write_message};
use crate::ServerEvent;

// ─── Error ───────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum LspError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Protocol(String),
    Timeout,
}

impl std::fmt::Display for LspError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "LSP I/O error: {e}"),
            Self::Json(e) => write!(f, "LSP JSON error: {e}"),
            Self::Protocol(s) => write!(f, "LSP protocol error: {s}"),
            Self::Timeout => write!(f, "LSP request timed out"),
        }
    }
}
impl std::error::Error for LspError {}

impl From<std::io::Error> for LspError {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}
impl From<serde_json::Error> for LspError {
    fn from(e: serde_json::Error) -> Self { Self::Json(e) }
}

// ─── LspClient ───────────────────────────────────────────────────────────────

type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, Value>>>>>;

/// Client handle for one language server process.
///
/// All request methods are `async` and communicate through internal tokio
/// channels so the reader/writer tasks own the actual I/O.
pub struct LspClient {
    next_id: Arc<AtomicU64>,
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    pending: PendingMap,
    /// Receive server-originated events (diagnostics, progress…).
    pub events: mpsc::UnboundedReceiver<ServerEvent>,
    // Keep child alive.
    _child: Child,
}

impl LspClient {
    /// Spawn `cmd` with `args`, start reader + writer background tasks, and
    /// send `initialize`.  Returns the ready client.
    pub async fn spawn(
        cmd: &Path,
        args: &[String],
        root_path: &Path,
    ) -> Result<Self, LspError> {
        let mut child = Command::new(cmd)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        let mut stdin: ChildStdin = child
            .stdin
            .take()
            .ok_or_else(|| LspError::Protocol("no stdin on child".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LspError::Protocol("no stdout on child".into()))?;
        let mut stdout_reader = BufReader::new(stdout);

        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let pending_r = pending.clone();

        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let (event_tx, events) = mpsc::unbounded_channel::<ServerEvent>();

        // ── Writer task ────────────────────────────────────────────────────
        tokio::spawn(async move {
            while let Some(msg) = write_rx.recv().await {
                if let Err(e) = write_message(&mut stdin, &msg).await {
                    warn!("LSP write error: {e}");
                    break;
                }
            }
        });

        // ── Reader task ────────────────────────────────────────────────────
        tokio::spawn(async move {
            loop {
                let raw = match read_message(&mut stdout_reader).await {
                    Ok(r) => r,
                    Err(e) => {
                        warn!("LSP read error: {e}");
                        break;
                    }
                };
                let val: Value = match serde_json::from_slice(&raw) {
                    Ok(v) => v,
                    Err(e) => { warn!("LSP JSON parse error: {e}"); continue; }
                };
                dispatch_message(val, &pending_r, &event_tx);
            }
        });

        let next_id = Arc::new(AtomicU64::new(1));
        let mut client = Self { next_id, write_tx, pending, events, _child: child };

        // Send initialize.
        let root_uri = path_to_uri(root_path);
        client.do_initialize(root_uri).await?;

        Ok(client)
    }

    // ── LSP lifecycle ─────────────────────────────────────────────────────

    async fn do_initialize(&mut self, root_uri: Option<Uri>) -> Result<(), LspError> {
        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_uri,
            capabilities: ClientCapabilities {
                workspace: Some(WorkspaceClientCapabilities {
                    workspace_folders: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let _result = self.request::<Initialize>(params).await?;
        // Send initialized notification (no response expected).
        self.notify::<Initialized>(InitializedParams {})?;
        Ok(())
    }

    /// Send LSP shutdown + exit.
    pub async fn shutdown(&mut self) -> Result<(), LspError> {
        let _ = self.request::<Shutdown>(()).await;
        self.send_raw(make_notification::<lsp_types::notification::Exit>(()))
    }

    // ── Text-document sync ────────────────────────────────────────────────

    pub fn did_open(&self, uri: Uri, language_id: String, version: i32, text: String) -> Result<(), LspError> {
        self.notify::<DidOpenTextDocument>(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id,
                version,
                text,
            },
        })
    }

    pub fn did_change(&self, uri: Uri, version: i32, text: String) -> Result<(), LspError> {
        self.notify::<DidChangeTextDocument>(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text,
            }],
        })
    }

    pub fn did_close(&self, uri: Uri) -> Result<(), LspError> {
        self.notify::<DidCloseTextDocument>(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
        })
    }

    // ── Requests ──────────────────────────────────────────────────────────

    pub async fn hover(
        &mut self,
        uri: Uri,
        position: Position,
    ) -> Result<Option<Hover>, LspError> {
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: Default::default(),
        };
        let val = self.request::<HoverRequest>(params).await?;
        let hover: Option<Hover> = serde_json::from_value(val)?;
        Ok(hover)
    }

    pub async fn completion(
        &mut self,
        uri: Uri,
        position: Position,
    ) -> Result<Option<CompletionResponse>, LspError> {
        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::INVOKED,
                trigger_character: None,
            }),
        };
        let val = self.request::<Completion>(params).await?;
        let resp: Option<CompletionResponse> = serde_json::from_value(val)?;
        Ok(resp)
    }

    pub async fn goto_definition(
        &mut self,
        uri: Uri,
        position: Position,
    ) -> Result<Option<GotoDefinitionResponse>, LspError> {
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        let val = self.request::<GotoDefinition>(params).await?;
        let resp: Option<GotoDefinitionResponse> = serde_json::from_value(val)?;
        Ok(resp)
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    /// Send a request and await the response.
    async fn request<R: lsp_types::request::Request>(
        &mut self,
        params: R::Params,
    ) -> Result<Value, LspError>
    where
        R::Params: serde::Serialize,
    {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let body = serde_json::to_vec(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": R::METHOD,
            "params": params,
        }))?;

        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            map.insert(id, tx);
        }

        self.send_raw(body)?;

        match tokio::time::timeout(
            std::time::Duration::from_secs(10),
            rx,
        )
        .await
        {
            Ok(Ok(Ok(val))) => Ok(val),
            Ok(Ok(Err(err))) => Err(LspError::Protocol(format!("{err}"))),
            Ok(Err(_)) => Err(LspError::Protocol("response channel dropped".into())),
            Err(_) => {
                // Remove pending entry on timeout.
                let mut map = self.pending.lock().unwrap_or_else(|e| e.into_inner());
                map.remove(&id);
                Err(LspError::Timeout)
            }
        }
    }

    /// Send a notification (no response expected).
    fn notify<N: lsp_types::notification::Notification>(
        &self,
        params: N::Params,
    ) -> Result<(), LspError>
    where
        N::Params: serde::Serialize,
    {
        self.send_raw(make_notification::<N>(params))
    }

    fn send_raw(&self, body: Vec<u8>) -> Result<(), LspError> {
        self.write_tx.send(body).map_err(|_| LspError::Protocol("writer channel closed".into()))
    }
}

// ─── Message dispatch ────────────────────────────────────────────────────────

fn dispatch_message(
    val: Value,
    pending: &PendingMap,
    event_tx: &mpsc::UnboundedSender<ServerEvent>,
) {
    // Response to a request: has "id" but no "method".
    if let Some(id_val) = val.get("id") {
        if val.get("method").is_none() {
            if let Some(id) = id_val.as_u64() {
                let mut map = pending.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(tx) = map.remove(&id) {
                    let result = if let Some(result) = val.get("result") {
                        Ok(result.clone())
                    } else if let Some(err) = val.get("error") {
                        Err(err.clone())
                    } else {
                        Ok(Value::Null)
                    };
                    let _ = tx.send(result);
                }
                return;
            }
        }
    }

    // Server-originated notification: has "method".
    if let Some(method) = val.get("method").and_then(Value::as_str) {
        debug!(method, "LSP notification");
        match method {
            "textDocument/publishDiagnostics" => {
                if let Some(params) = val.get("params") {
                    if let Ok(diags) = serde_json::from_value::<
                        lsp_types::PublishDiagnosticsParams,
                    >(params.clone())
                    {
                        let _ = event_tx.send(ServerEvent::Diagnostics(diags));
                    }
                }
            }
            "window/logMessage" | "window/showMessage" => {
                if let Some(msg) = val.get("params")
                    .and_then(|p| p.get("message"))
                    .and_then(Value::as_str)
                {
                    debug!("LSP message: {msg}");
                }
            }
            _ => {}
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_notification<N: lsp_types::notification::Notification>(
    params: N::Params,
) -> Vec<u8>
where
    N::Params: serde::Serialize,
{
    serde_json::to_vec(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": N::METHOD,
        "params": params,
    }))
    .unwrap_or_default()
}

fn path_to_uri(path: &Path) -> Option<Uri> {
    // Construct a file:// URI string and parse it.  Works regardless of the
    // underlying URI type used by lsp_types (url::Url or fluent_uri::Uri).
    let abs = std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf());
    let s = abs.to_str()?;
    let uri_str = if cfg!(windows) {
        format!("file:///{}", s.replace('\\', "/"))
    } else {
        format!("file://{s}")
    };
    uri_str.parse().ok()
}
