use anyhow::Context as _;
use fs2::FileExt as _;
use std::{fs::OpenOptions, path::Path};

#[derive(Debug)]
pub struct SingleInstanceLock {
    _file: std::fs::File,
    path: std::path::PathBuf,
}

impl SingleInstanceLock {
    pub fn acquire(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("failed to open lock file {}", path.display()))?;

        // Cross-platform advisory lock (Linux/macOS/Windows). This is much safer than using
        // "create file" as a lock, because OS releases the lock if the process dies.
        match file.try_lock_exclusive() {
            Ok(()) => Ok(Self { _file: file, path }),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Err(anyhow::anyhow!(
                "another rust-mule instance appears to be running (lock held at {})",
                path.display()
            )),
            Err(e) => Err(anyhow::anyhow!(e))
                .with_context(|| format!("failed to lock {}", path.display())),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
