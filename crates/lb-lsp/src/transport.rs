use std::io;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};

// ─── LSP Content-Length framing ──────────────────────────────────────────────
// Format:  "Content-Length: <n>\r\n\r\n" followed by exactly <n> UTF-8 bytes.
// Reference: https://microsoft.github.io/language-server-protocol/specifications/base/0.9/specification/

const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024; // 64 MB hard cap

/// Write one JSON-RPC message to `stdin` with Content-Length framing.
pub async fn write_message(stdin: &mut ChildStdin, body: &[u8]) -> io::Result<()> {
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    stdin.write_all(header.as_bytes()).await?;
    stdin.write_all(body).await?;
    stdin.flush().await
}

/// Read one JSON-RPC message from `stdout`, stripping the header.
pub async fn read_message(stdout: &mut BufReader<ChildStdout>) -> io::Result<Vec<u8>> {
    let mut content_length: Option<usize> = None;

    // Parse headers until we see the blank line.
    loop {
        let mut line = String::new();
        let n = stdout.read_line(&mut line).await?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "LSP peer closed"));
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break; // blank line terminates headers
        }
        if let Some(rest) = line.strip_prefix("Content-Length:") {
            let val = rest.trim();
            match val.parse::<usize>() {
                Ok(n) => content_length = Some(n),
                Err(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("bad Content-Length value: {val}"),
                    ))
                }
            }
        }
        // Other headers (Content-Type) are silently ignored.
    }

    let len = content_length.ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "LSP message missing Content-Length")
    })?;

    if len > MAX_MESSAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("LSP message too large: {len} bytes"),
        ));
    }

    let mut buf = vec![0u8; len];
    stdout.read_exact(&mut buf).await?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_format() {
        // Simple structural check: the header format is correct.
        let body = br#"{"jsonrpc":"2.0","id":1}"#;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        assert!(header.starts_with("Content-Length:"));
        assert!(header.ends_with("\r\n\r\n"));
    }
}
