use std::path::Path;

pub type Result<T> = std::result::Result<T, SamKeysError>;

#[derive(Debug)]
pub enum SamKeysError {
    InvalidLine(String),
    UnknownKey(String),
    MissingPub,
    MissingPriv,
    Read {
        path: String,
        source: std::io::Error,
    },
    Utf8 {
        path: String,
        source: std::string::FromUtf8Error,
    },
    Parse {
        path: String,
        source: Box<SamKeysError>,
    },
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

impl std::fmt::Display for SamKeysError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLine(line) => write!(f, "invalid sam.keys line (expected k=v): {line}"),
            Self::UnknownKey(k) => write!(f, "unknown sam.keys key: {k}"),
            Self::MissingPub => write!(f, "sam.keys missing PUB="),
            Self::MissingPriv => write!(f, "sam.keys missing PRIV="),
            Self::Read { path, .. } => write!(f, "failed to read {path}"),
            Self::Utf8 { path, .. } => write!(f, "{path} was not valid UTF-8"),
            Self::Parse { path, .. } => write!(f, "failed to parse {path}"),
            Self::CreateDir { path, .. } => write!(f, "failed to create {path}"),
            Self::Write { path, .. } => write!(f, "failed to write {path}"),
            Self::Rename { from, to, .. } => write!(f, "failed to rename {from} -> {to}"),
        }
    }
}

impl std::error::Error for SamKeysError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Utf8 { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::CreateDir { source, .. } => Some(source),
            Self::Write { source, .. } => Some(source),
            Self::Rename { source, .. } => Some(source),
            Self::InvalidLine(_) | Self::UnknownKey(_) | Self::MissingPub | Self::MissingPriv => {
                None
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SamKeys {
    pub pub_key: String,
    pub priv_key: String,
}

impl SamKeys {
    pub fn parse(text: &str) -> Result<Self> {
        let mut pub_key: Option<String> = None;
        let mut priv_key: Option<String> = None;

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                return Err(SamKeysError::InvalidLine(line.to_string()));
            };
            let k = k.trim().to_ascii_uppercase();
            let v = v.trim().to_string();
            match k.as_str() {
                "PUB" => pub_key = Some(v),
                "PRIV" => priv_key = Some(v),
                other => return Err(SamKeysError::UnknownKey(other.to_string())),
            }
        }

        let pub_key = pub_key.ok_or(SamKeysError::MissingPub)?;
        let priv_key = priv_key.ok_or(SamKeysError::MissingPriv)?;
        Ok(Self { pub_key, priv_key })
    }

    pub fn to_string_kv(&self) -> String {
        // Keep it simple and greppable.
        format!(
            "PUB={}\nPRIV={}\n",
            self.pub_key.trim(),
            self.priv_key.trim()
        )
    }

    pub async fn load(path: &Path) -> Result<Option<Self>> {
        let bytes = match tokio::fs::read(path).await {
            Ok(b) => b,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(SamKeysError::Read {
                    path: path.display().to_string(),
                    source,
                });
            }
        };
        let text = String::from_utf8(bytes).map_err(|source| SamKeysError::Utf8 {
            path: path.display().to_string(),
            source,
        })?;
        Ok(Some(Self::parse(&text).map_err(|source| {
            SamKeysError::Parse {
                path: path.display().to_string(),
                source: Box::new(source),
            }
        })?))
    }

    pub async fn store(path: &Path, keys: &SamKeys) -> Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|source| SamKeysError::CreateDir {
                    path: parent.display().to_string(),
                    source,
                })?;
        }

        let tmp = path.with_extension("tmp");
        tokio::fs::write(&tmp, keys.to_string_kv())
            .await
            .map_err(|source| SamKeysError::Write {
                path: tmp.display().to_string(),
                source,
            })?;
        tokio::fs::rename(&tmp, path)
            .await
            .map_err(|source| SamKeysError::Rename {
                from: tmp.display().to_string(),
                to: path.display().to_string(),
                source,
            })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let perm = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(path, perm);
        }

        Ok(())
    }
}
