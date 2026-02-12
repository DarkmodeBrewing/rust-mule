use fs2::FileExt as _;
use std::{fs::OpenOptions, path::Path};

pub type Result<T> = std::result::Result<T, SingleInstanceError>;

#[derive(Debug)]
pub enum SingleInstanceError {
    CreateDir {
        path: String,
        source: std::io::Error,
    },
    Open {
        path: String,
        source: std::io::Error,
    },
    AlreadyRunning {
        path: String,
    },
    Lock {
        path: String,
        source: std::io::Error,
    },
}

impl std::fmt::Display for SingleInstanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateDir { path, .. } => write!(f, "failed to create {path}"),
            Self::Open { path, .. } => write!(f, "failed to open lock file {path}"),
            Self::AlreadyRunning { path } => write!(
                f,
                "another rust-mule instance appears to be running (lock held at {path})"
            ),
            Self::Lock { path, .. } => write!(f, "failed to lock {path}"),
        }
    }
}

impl std::error::Error for SingleInstanceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CreateDir { source, .. } => Some(source),
            Self::Open { source, .. } => Some(source),
            Self::Lock { source, .. } => Some(source),
            Self::AlreadyRunning { .. } => None,
        }
    }
}

#[derive(Debug)]
pub struct SingleInstanceLock {
    _file: std::fs::File,
    path: std::path::PathBuf,
}

impl SingleInstanceLock {
    pub fn acquire(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| SingleInstanceError::CreateDir {
                path: parent.display().to_string(),
                source,
            })?;
        }

        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|source| SingleInstanceError::Open {
                path: path.display().to_string(),
                source,
            })?;

        // Cross-platform advisory lock (Linux/macOS/Windows). This is much safer than using
        // "create file" as a lock, because OS releases the lock if the process dies.
        match file.try_lock_exclusive() {
            Ok(()) => Ok(Self { _file: file, path }),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                Err(SingleInstanceError::AlreadyRunning {
                    path: path.display().to_string(),
                })
            }
            Err(source) => Err(SingleInstanceError::Lock {
                path: path.display().to_string(),
                source,
            }),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
