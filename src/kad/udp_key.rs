use std::path::Path;

pub type Result<T> = std::result::Result<T, UdpKeyError>;

#[derive(Debug)]
pub enum UdpKeyError {
    WrongSize {
        path: String,
        len: usize,
    },
    Random(String),
    CreateDir {
        path: String,
        source: std::io::Error,
    },
    Write {
        path: String,
        source: std::io::Error,
    },
    Rename {
        from: String,
        to: String,
        source: std::io::Error,
    },
}

impl std::fmt::Display for UdpKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongSize { path, len } => write!(
                f,
                "kad udp key secret file has wrong size: {} bytes (expected 4): {}",
                len, path
            ),
            Self::Random(msg) => write!(f, "{msg}"),
            Self::CreateDir { path, .. } => write!(f, "failed creating directory {path}"),
            Self::Write { path, .. } => write!(f, "failed to write {path}"),
            Self::Rename { from, to, .. } => write!(f, "failed to rename {from} -> {to}"),
        }
    }
}

impl std::error::Error for UdpKeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CreateDir { source, .. } => Some(source),
            Self::Write { source, .. } => Some(source),
            Self::Rename { source, .. } => Some(source),
            Self::WrongSize { .. } | Self::Random(_) => None,
        }
    }
}

/// Load or create our local Kad UDP "key secret" (iMule `thePrefs::GetKadUDPKey()` equivalent).
///
/// We keep this **out of** `config.toml` so running the app doesn't mutate a tracked file.
/// The secret only needs to be stable for the lifetime of a node identity; storing it under
/// `data/` achieves that.
pub async fn load_or_create_udp_key_secret(path: &Path) -> Result<u32> {
    if let Ok(bytes) = tokio::fs::read(path).await {
        if bytes.len() != 4 {
            return Err(UdpKeyError::WrongSize {
                path: path.display().to_string(),
                len: bytes.len(),
            });
        }
        let v = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        // 0 is reserved as "unset" in config; avoid persisting it as a real value.
        if v != 0 {
            return Ok(v);
        }
    }

    let v: u32 = loop {
        let mut b = [0u8; 4];
        getrandom::getrandom(&mut b).map_err(|e| {
            UdpKeyError::Random(format!("failed to generate kad udp key secret: {e}"))
        })?;
        let v = u32::from_le_bytes(b);
        if v != 0 {
            break v;
        }
    };

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|source| UdpKeyError::CreateDir {
                path: parent.display().to_string(),
                source,
            })?;
    }

    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, v.to_le_bytes())
        .await
        .map_err(|source| UdpKeyError::Write {
            path: tmp.display().to_string(),
            source,
        })?;
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|source| UdpKeyError::Rename {
            from: tmp.display().to_string(),
            to: path.display().to_string(),
            source,
        })?;

    Ok(v)
}
