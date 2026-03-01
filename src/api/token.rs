use getrandom::getrandom;
use std::path::Path;

pub type Result<T> = std::result::Result<T, TokenError>;

#[derive(Debug)]
pub enum TokenError {
    GetRandom(String),
    WriteTemp(std::io::Error),
    Rename(std::io::Error),
    InvalidPath(String),
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GetRandom(msg) => write!(f, "getrandom failed: {msg}"),
            Self::WriteTemp(_) => write!(f, "failed to write temporary token file"),
            Self::Rename(_) => write!(f, "failed to atomically rename temporary token file"),
            Self::InvalidPath(msg) => write!(f, "invalid token path: {msg}"),
        }
    }
}

impl std::error::Error for TokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::WriteTemp(source) => Some(source),
            Self::Rename(source) => Some(source),
            Self::GetRandom(_) => None,
            Self::InvalidPath(_) => None,
        }
    }
}

/// Sanitize and constrain the token path to live under a controlled base directory.
fn sanitize_token_path(path: &Path) -> Result<std::path::PathBuf> {
    use std::env;
    use std::path::{PathBuf};

    // Allow overriding the base directory via environment for flexibility; default to CWD.
    let base = env::var("TOKEN_BASE_DIR").unwrap_or_else(|_| String::from("."));
    let base_path = PathBuf::from(base);

    // Join the potentially untrusted path to the base directory.
    let candidate = base_path.join(path);

    // Canonicalize to eliminate any `..` components, symlinks, etc.
    let canonical = candidate
        .canonicalize()
        .or_else(|_| {
            // If the file does not yet exist, canonicalize the parent directory instead.
            if let Some(parent) = candidate.parent() {
                let parent_canon = parent.canonicalize()?;
                Ok(parent_canon.join(
                    candidate
                        .file_name()
                        .ok_or_else(|| {
                            std::io::Error::new(
                                std::io::ErrorKind::Other,
                                "token path has no file name",
                            )
                        })?,
                ))
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "token path has no parent directory",
                ))
            }
        })
        .map_err(|e| TokenError::InvalidPath(e.to_string()))?;

    if !canonical.starts_with(&base_path) {
        return Err(TokenError::InvalidPath(
            "resolved path escapes TOKEN_BASE_DIR".to_string(),
        ));
    }

    Ok(canonical)
}

pub async fn load_or_create_token(path: &Path) -> Result<String> {
    let safe_path = sanitize_token_path(path)?;

    if let Ok(bytes) = tokio::fs::read(&safe_path).await {
        match String::from_utf8(bytes) {
            Ok(s) => {
                let s = s.trim().to_string();
                if is_valid_token(&s) {
                    return Ok(s);
                }
                tracing::warn!("token file was invalid/empty; rotating token");
            }
            Err(err) => {
                tracing::warn!(error = %err, "token file was invalid UTF-8; rotating token");
            }
        }
    }

    if let Some(parent) = safe_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }

    rotate_token(&safe_path).await
}

fn is_valid_token(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn hex_lower(b: &[u8]) -> String {
    use std::fmt::Write as _;
    let safe_path = sanitize_token_path(path)?;

    let mut out = String::with_capacity(b.len() * 2);
    for v in b {
        let _ = write!(&mut out, "{v:02x}");
    }
    out
}

pub async fn rotate_token(path: &Path) -> Result<String> {
    let mut raw = [0u8; 32];
    getrandom(&mut raw).map_err(|e| TokenError::GetRandom(format!("{e:?}")))?;
    let token = hex_lower(&raw);

    let tmp = safe_path.with_extension("tmp");

    // Apply restrictive permissions to the temp file *before* the atomic rename so that
    // the final file is never visible to other users, even briefly.
    #[cfg(unix)]
    {
        let mut opts = tokio::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true).mode(0o600);
        let mut f = opts.open(&tmp).await.map_err(TokenError::WriteTemp)?;
        use tokio::io::AsyncWriteExt as _;
        f.write_all(token.as_bytes())
            .await
            .map_err(TokenError::WriteTemp)?;
    }
    #[cfg(not(unix))]
    {
        tokio::fs::write(&tmp, token.as_bytes())
            .await
            .map_err(TokenError::WriteTemp)?;
    }

    tokio::fs::rename(&tmp, &safe_path)
        .await
        .map_err(TokenError::Rename)?;

    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_token_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "rust_mule_token_test_{}_{}_{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    #[tokio::test]
    async fn load_or_create_token_rotates_invalid_utf8_file() {
        let path = temp_token_path("utf8");
        tokio::fs::write(&path, [0xff, 0xfe, 0xfd])
            .await
            .expect("write invalid token");
        let token = load_or_create_token(&path).await.expect("rotate");
        assert!(is_valid_token(&token));
        let persisted = tokio::fs::read_to_string(&path).await.expect("read token");
        assert_eq!(token, persisted);
        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn load_or_create_token_rotates_non_hex_file() {
        let path = temp_token_path("nonhex");
        tokio::fs::write(&path, "not-a-token")
            .await
            .expect("write invalid token");
        let token = load_or_create_token(&path).await.expect("rotate");
        assert!(is_valid_token(&token));
        let persisted = tokio::fs::read_to_string(&path).await.expect("read token");
        assert_eq!(token, persisted);
        let _ = tokio::fs::remove_file(&path).await;
    }
}
