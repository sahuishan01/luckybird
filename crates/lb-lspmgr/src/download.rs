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
    Rejected(&'static str),
}

/// Upper bound on a downloaded archive; protects against unbounded memory use.
const MAX_DOWNLOAD_BYTES: u64 = 512 * 1024 * 1024;

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Http(e) => write!(f, "HTTP error: {e}"),
            Self::Checksum { expected, got } => {
                write!(f, "checksum mismatch: expected {expected}, got {got}")
            }
            Self::Extract(e) => write!(f, "extraction error: {e}"),
            Self::Rejected(why) => write!(f, "download rejected: {why}"),
        }
    }
}
impl std::error::Error for DownloadError {}

impl From<io::Error> for DownloadError {
    fn from(e: io::Error) -> Self { Self::Io(e) }
}

// ─── Download + verify + extract ─────────────────────────────────────────────

/// Download `url` over https, verify its SHA-256 (mandatory), then extract
/// into `dest_dir`.
///
/// `archive_format` controls extraction:
/// - `"gz"` — single-file gzip (extracts to `<dest_dir>/<binary_name>`)
/// - `"zip"` — zip archive
/// - `"tar.gz"` — gzip-compressed tar
/// - `""` — raw binary (no extraction, just copy)
pub fn download_and_install(
    url: &str,
    expected_sha256: &str,
    dest_dir: &Path,
    binary_name: &str,
    archive_format: &str,
) -> Result<PathBuf, DownloadError> {
    if !url.starts_with("https://") {
        return Err(DownloadError::Rejected("url must use https"));
    }
    let is_plain_name = Path::new(binary_name)
        .file_name()
        .is_some_and(|n| n == std::ffi::OsStr::new(binary_name));
    if !is_plain_name {
        return Err(DownloadError::Rejected("binary name must be a plain file name"));
    }

    info!(url, "downloading LSP server");

    let resp = ureq::get(url)
        .call()
        .map_err(|e| DownloadError::Http(e.to_string()))?;

    let mut data = Vec::new();
    let mut reader = io::Read::take(resp.into_reader(), MAX_DOWNLOAD_BYTES + 1);
    io::Read::read_to_end(&mut reader, &mut data).map_err(DownloadError::Io)?;
    if data.len() as u64 > MAX_DOWNLOAD_BYTES {
        return Err(DownloadError::Rejected("download exceeds size limit"));
    }

    debug!(bytes = data.len(), "download complete");

    let got = format!("{:x}", Sha256::digest(&data));
    if !got.eq_ignore_ascii_case(expected_sha256) {
        return Err(DownloadError::Checksum {
            expected: expected_sha256.to_string(),
            got,
        });
    }
    debug!("checksum OK");

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
    fn rejects_non_https_url() {
        let dir = std::env::temp_dir().join("lb_test_http");
        let r = download_and_install("http://example.com/x", "00", &dir, "x", "");
        assert!(matches!(r, Err(DownloadError::Rejected(_))));
    }

    #[test]
    fn rejects_path_in_binary_name() {
        let dir = std::env::temp_dir().join("lb_test_name");
        let r = download_and_install("https://example.com/x", "00", &dir, "../evil", "");
        assert!(matches!(r, Err(DownloadError::Rejected(_))));
    }

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
