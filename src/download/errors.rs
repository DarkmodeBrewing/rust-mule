#[derive(Debug)]
pub enum DownloadStoreError {
    EnsureDir {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    ReadDir {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    ReadFile {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    WriteFile {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    Rename {
        from: std::path::PathBuf,
        to: std::path::PathBuf,
        source: std::io::Error,
    },
    Copy {
        from: std::path::PathBuf,
        to: std::path::PathBuf,
        source: std::io::Error,
    },
    Serialize {
        source: serde_json::Error,
    },
    ParseMet {
        path: std::path::PathBuf,
        source: serde_json::Error,
    },
}

impl std::fmt::Display for DownloadStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EnsureDir { path, source } => {
                write!(
                    f,
                    "failed to ensure directory '{}': {source}",
                    path.display()
                )
            }
            Self::ReadDir { path, source } => {
                write!(f, "failed to read directory '{}': {source}", path.display())
            }
            Self::ReadFile { path, source } => {
                write!(f, "failed to read file '{}': {source}", path.display())
            }
            Self::WriteFile { path, source } => {
                write!(f, "failed to write file '{}': {source}", path.display())
            }
            Self::Rename { from, to, source } => write!(
                f,
                "failed to rename '{}' -> '{}': {source}",
                from.display(),
                to.display()
            ),
            Self::Copy { from, to, source } => write!(
                f,
                "failed to copy '{}' -> '{}': {source}",
                from.display(),
                to.display()
            ),
            Self::Serialize { source } => write!(f, "failed to serialize part metadata: {source}"),
            Self::ParseMet { path, source } => write!(
                f,
                "failed to parse part metadata '{}': {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for DownloadStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EnsureDir { source, .. } => Some(source),
            Self::ReadDir { source, .. } => Some(source),
            Self::ReadFile { source, .. } => Some(source),
            Self::WriteFile { source, .. } => Some(source),
            Self::Rename { source, .. } => Some(source),
            Self::Copy { source, .. } => Some(source),
            Self::Serialize { source } => Some(source),
            Self::ParseMet { source, .. } => Some(source),
        }
    }
}

#[derive(Debug)]
pub enum DownloadError {
    Store(DownloadStoreError),
    ChannelClosed,
    ServiceJoin(tokio::task::JoinError),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(source) => write!(f, "{source}"),
            Self::ChannelClosed => write!(f, "download service channel closed"),
            Self::ServiceJoin(source) => write!(f, "download service task join error: {source}"),
        }
    }
}

impl std::error::Error for DownloadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(source) => Some(source),
            Self::ServiceJoin(source) => Some(source),
            Self::ChannelClosed => None,
        }
    }
}

impl From<DownloadStoreError> for DownloadError {
    fn from(value: DownloadStoreError) -> Self {
        Self::Store(value)
    }
}

impl From<tokio::task::JoinError> for DownloadError {
    fn from(value: tokio::task::JoinError) -> Self {
        Self::ServiceJoin(value)
    }
}
