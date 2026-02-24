use getrandom::getrandom;
use std::path::Path;

pub type Result<T> = std::result::Result<T, TokenError>;

#[derive(Debug)]
pub enum TokenError {
    GetRandom(String),
    WriteTemp(std::io::Error),
    Rename(std::io::Error),
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GetRandom(msg) => write!(f, "getrandom failed: {msg}"),
            Self::WriteTemp(_) => write!(f, "failed to write temporary token file"),
            Self::Rename(_) => write!(f, "failed to atomically rename temporary token file"),
        }
    }
}

impl std::error::Error for TokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::WriteTemp(source) => Some(source),
            Self::Rename(source) => Some(source),
            Self::GetRandom(_) => None,
        }
    }
}

pub async fn load_or_create_token(path: &Path) -> Result<String> {
    if let Ok(bytes) = tokio::fs::read(path).await {
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

    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }

    rotate_token(path).await
}

fn is_valid_token(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn hex_lower(b: &[u8]) -> String {
    use std::fmt::Write as _;
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

    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, token.as_bytes())
        .await
        .map_err(TokenError::WriteTemp)?;
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(TokenError::Rename)?;

    // Best-effort to restrict secrets on Unix; ignore failures (still portable).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let perm = std::fs::Permissions::from_mode(0o600);
        let _ = std::fs::set_permissions(path, perm);
    }

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
