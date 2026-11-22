use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tracing::{debug, info};

// ─── Error ───────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum DownloadError {
    Io(io::Error),
    Http(String),
    Checksum { expected: String, got: String },
    Extract(String),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Http(e) => write!(f, "HTTP error: {e}"),
            Self::Checksum { expected, got } => {
                write!(f, "checksum mismatch: expected {expected}, got {got}")
            }
            Self::Extract(e) => write!(f, "extraction error: {e}"),
        }
    }
}
impl std::error::Error for DownloadError {}

impl From<io::Error> for DownloadError {
    fn from(e: io::Error) -> Self { Self::Io(e) }
}

// ─── Download + verify + extract ─────────────────────────────────────────────

/// Download `url` to a temp file, verify the SHA-256 if provided, then
/// extract into `dest_dir`.
///
/// `archive_format` controls extraction:
/// - `"gz"` — single-file gzip (extracts to `<dest_dir>/<binary_name>`)
/// - `"zip"` — zip archive
/// - `"tar.gz"` — gzip-compressed tar
/// - `""` — raw binary (no extraction, just copy)
pub fn download_and_install(
    url: &str,
    expected_sha256: Option<&str>,
    dest_dir: &Path,
    binary_name: &str,
    archive_format: &str,
) -> Result<PathBuf, DownloadError> {
    info!(url, "downloading LSP server");

    // Fetch into memory (ureq blocking).
    let resp = ureq::get(url)
        .call()
        .map_err(|e| DownloadError::Http(e.to_string()))?;

    let mut data = Vec::new();
    let mut reader = resp.into_reader();
    io::Read::read_to_end(&mut reader, &mut data)
        .map_err(DownloadError::Io)?;

    debug!(bytes = data.len(), "download complete");

    // Verify checksum.
    if let Some(expected) = expected_sha256 {
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let got = format!("{:x}", hasher.finalize());
        if got != expected {
            return Err(DownloadError::Checksum {
                expected: expected.to_string(),
                got,
            });
        }
        debug!("checksum OK");
    }

    // Ensure destination directory exists.
    fs::create_dir_all(dest_dir)?;

    // Extract or copy.
    let binary_path = match archive_format {
        "gz" => extract_gz(&data, dest_dir, binary_name)?,
        "zip" => extract_zip(&data, dest_dir, binary_name)?,
        "tar.gz" | "tgz" => extract_tar_gz(&data, dest_dir, binary_name)?,
        _ => write_raw(&data, dest_dir, binary_name)?,
    };

    // Make executable on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&binary_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&binary_path, perms)?;
    }

    info!(path = %binary_path.display(), "LSP server installed");
    Ok(binary_path)
}

// ─── Extraction helpers ───────────────────────────────────────────────────────

fn extract_gz(data: &[u8], dest_dir: &Path, binary_name: &str) -> Result<PathBuf, DownloadError> {
    use flate2::read::GzDecoder;

    let mut decoder = GzDecoder::new(data);
    let mut out = Vec::new();
    io::Read::read_to_end(&mut decoder, &mut out).map_err(DownloadError::Io)?;
    write_raw(&out, dest_dir, binary_name)
}

fn extract_zip(data: &[u8], dest_dir: &Path, binary_name: &str) -> Result<PathBuf, DownloadError> {
    use std::io::Cursor;
    use zip::ZipArchive;

    let cursor = Cursor::new(data);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| DownloadError::Extract(e.to_string()))?;

    // Find and extract the target binary.
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| DownloadError::Extract(e.to_string()))?;
        let name = file.name().to_string();
        // Match by filename (last path component).
        let base = name.rsplit('/').next().unwrap_or(&name);
        let base_stripped = base.strip_suffix(".exe").unwrap_or(base);
        let target_stripped = binary_name.strip_suffix(".exe").unwrap_or(binary_name);
        if base_stripped == target_stripped {
            let dest = dest_dir.join(binary_name);
            let mut out = fs::File::create(&dest)?;
            io::copy(&mut file, &mut out)?;
            return Ok(dest);
        }
    }

    Err(DownloadError::Extract(format!(
        "binary {binary_name} not found in zip archive"
    )))
}

fn extract_tar_gz(
    data: &[u8],
    dest_dir: &Path,
    binary_name: &str,
) -> Result<PathBuf, DownloadError> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);

    let target_stripped = binary_name.strip_suffix(".exe").unwrap_or(binary_name);
    for entry in archive.entries().map_err(DownloadError::Io)? {
        let mut entry = entry.map_err(DownloadError::Io)?;
        let path = entry.path().map_err(DownloadError::Io)?.to_path_buf();
        let base = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let base_stripped = base.strip_suffix(".exe").unwrap_or(base);
        if base_stripped == target_stripped {
            let dest = dest_dir.join(binary_name);
            entry.unpack(&dest).map_err(DownloadError::Io)?;
            return Ok(dest);
        }
    }

    Err(DownloadError::Extract(format!(
        "binary {binary_name} not found in tar archive"
    )))
}

fn write_raw(data: &[u8], dest_dir: &Path, binary_name: &str) -> Result<PathBuf, DownloadError> {
    let dest = dest_dir.join(binary_name);
    let mut file = fs::File::create(&dest)?;
    file.write_all(data)?;
    Ok(dest)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_raw_creates_file() {
        let dir = std::env::temp_dir().join("lb_test_raw");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = write_raw(b"hello", &dir, "testbin").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"hello");
        let _ = fs::remove_dir_all(&dir);
    }
}
