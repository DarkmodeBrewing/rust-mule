#[derive(Debug)]
pub enum DownloadStoreError {
    EnsureDir {
        path: std::path::PathBuf,
        source: std::io::Error,
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
        }
    }
}

impl std::error::Error for DownloadStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::EnsureDir { source, .. } => Some(source),
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
