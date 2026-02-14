use crate::download::errors::{DownloadError, DownloadStoreError};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::sync::{mpsc, oneshot, watch};

pub type Result<T> = std::result::Result<T, DownloadError>;

#[derive(Debug, Clone)]
pub struct DownloadServiceConfig {
    pub download_dir: PathBuf,
    pub incoming_dir: PathBuf,
}

impl DownloadServiceConfig {
    pub fn from_data_dir(data_dir: &Path) -> Self {
        Self {
            download_dir: data_dir.join("download"),
            incoming_dir: data_dir.join("incoming"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadServiceStatus {
    pub running: bool,
    pub queue_len: usize,
    pub started_at: Instant,
}

#[derive(Debug)]
pub enum DownloadCommand {
    Ping { reply: oneshot::Sender<()> },
    Shutdown { reply: oneshot::Sender<()> },
}

#[derive(Clone)]
pub struct DownloadServiceHandle {
    tx: mpsc::Sender<DownloadCommand>,
}

impl DownloadServiceHandle {
    pub async fn ping(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(DownloadCommand::Ping { reply: tx })
            .await
            .map_err(|_| DownloadError::ChannelClosed)?;
        rx.await.map_err(|_| DownloadError::ChannelClosed)?;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(DownloadCommand::Shutdown { reply: tx })
            .await
            .map_err(|_| DownloadError::ChannelClosed)?;
        rx.await.map_err(|_| DownloadError::ChannelClosed)?;
        Ok(())
    }
}

pub async fn start_service(
    cfg: DownloadServiceConfig,
) -> Result<(
    DownloadServiceHandle,
    watch::Receiver<DownloadServiceStatus>,
    tokio::task::JoinHandle<Result<()>>,
)> {
    ensure_dirs(&cfg).await?;
    let (tx, rx) = mpsc::channel(128);
    let started_at = Instant::now();
    let (status_tx, status_rx) = watch::channel(DownloadServiceStatus {
        running: true,
        queue_len: 0,
        started_at,
    });
    let join = tokio::spawn(run_service(rx, status_tx, started_at));
    Ok((DownloadServiceHandle { tx }, status_rx, join))
}

async fn ensure_dirs(cfg: &DownloadServiceConfig) -> Result<()> {
    tokio::fs::create_dir_all(&cfg.download_dir)
        .await
        .map_err(|source| DownloadStoreError::EnsureDir {
            path: cfg.download_dir.clone(),
            source,
        })?;
    tokio::fs::create_dir_all(&cfg.incoming_dir)
        .await
        .map_err(|source| DownloadStoreError::EnsureDir {
            path: cfg.incoming_dir.clone(),
            source,
        })?;
    Ok(())
}

async fn run_service(
    mut rx: mpsc::Receiver<DownloadCommand>,
    status_tx: watch::Sender<DownloadServiceStatus>,
    started_at: Instant,
) -> Result<()> {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            DownloadCommand::Ping { reply } => {
                let _ = reply.send(());
            }
            DownloadCommand::Shutdown { reply } => {
                let _ = reply.send(());
                let _ = status_tx.send(DownloadServiceStatus {
                    running: false,
                    queue_len: 0,
                    started_at,
                });
                return Ok(());
            }
        }
    }

    let _ = status_tx.send(DownloadServiceStatus {
        running: false,
        queue_len: 0,
        started_at,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        p.push(format!("rust-mule-download-{tag}-{nanos}"));
        p
    }

    #[tokio::test]
    async fn start_service_creates_download_and_incoming_dirs() {
        let root = temp_dir("dirs");
        let cfg = DownloadServiceConfig::from_data_dir(&root);
        let (handle, _status_rx, join) = start_service(cfg.clone()).await.expect("start");

        assert!(cfg.download_dir.exists());
        assert!(cfg.incoming_dir.exists());

        handle.shutdown().await.expect("shutdown");
        join.await.expect("join").expect("service");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn service_ping_and_shutdown_flow() {
        let root = temp_dir("flow");
        let cfg = DownloadServiceConfig::from_data_dir(&root);
        let (handle, mut status_rx, join) = start_service(cfg).await.expect("start");

        handle.ping().await.expect("ping");
        assert!(status_rx.borrow().running);

        handle.shutdown().await.expect("shutdown");
        status_rx.changed().await.expect("status changed");
        assert!(!status_rx.borrow().running);

        join.await.expect("join").expect("service");
        let _ = std::fs::remove_dir_all(&root);
    }
}
