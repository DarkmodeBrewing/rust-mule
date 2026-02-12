use getrandom::getrandom;
use std::path::Path;

pub type Result<T> = std::result::Result<T, TokenError>;

#[derive(Debug)]
pub enum TokenError {
    Utf8(std::string::FromUtf8Error),
    GetRandom(String),
    WriteTemp(std::io::Error),
    Rename(std::io::Error),
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Utf8(_) => write!(f, "token file was not valid UTF-8"),
            Self::GetRandom(msg) => write!(f, "getrandom failed: {msg}"),
            Self::WriteTemp(_) => write!(f, "failed to write temporary token file"),
            Self::Rename(_) => write!(f, "failed to atomically rename temporary token file"),
        }
    }
}

impl std::error::Error for TokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Utf8(source) => Some(source),
            Self::WriteTemp(source) => Some(source),
            Self::Rename(source) => Some(source),
            Self::GetRandom(_) => None,
        }
    }
}

pub async fn load_or_create_token(path: &Path) -> Result<String> {
    if let Ok(bytes) = tokio::fs::read(path).await {
        let s = String::from_utf8(bytes).map_err(TokenError::Utf8)?;
        let s = s.trim().to_string();
        if !s.is_empty() {
            return Ok(s);
        }
    }

    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }

    rotate_token(path).await
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
