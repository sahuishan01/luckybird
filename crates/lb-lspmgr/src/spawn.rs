use std::path::{Path, PathBuf};

use lb_lsp::LspClient;
use tracing::info;

use crate::registry::ServerMeta;

// ─── Error ───────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum SpawnError {
    Lsp(lb_lsp::LspError),
}

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lsp(e) => write!(f, "LSP client error: {e}"),
        }
    }
}
impl std::error::Error for SpawnError {}

impl From<lb_lsp::LspError> for SpawnError {
    fn from(e: lb_lsp::LspError) -> Self { Self::Lsp(e) }
}

// ─── Spawn ───────────────────────────────────────────────────────────────────

/// Spawn a language server and return the connected `LspClient`.
///
/// `binary` must be an absolute path or a binary on PATH.
/// `root_path` is the workspace root sent in `initialize`.
pub async fn spawn_server(
    binary: &Path,
    meta: &ServerMeta,
    root_path: &Path,
) -> Result<LspClient, SpawnError> {
    info!(
        binary = %binary.display(),
        root = %root_path.display(),
        "spawning LSP server"
    );
    let client = LspClient::spawn(binary, &meta.args, root_path).await?;
    Ok(client)
}

/// Resolve a candidate binary path from the server meta and spawn it.
pub async fn spawn_from_path(
    binary_path: PathBuf,
    meta: &ServerMeta,
    root_path: &Path,
) -> Result<LspClient, SpawnError> {
    spawn_server(&binary_path, meta, root_path).await
}
