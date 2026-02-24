use crate::download::errors::{DownloadError, DownloadStoreError};
use crate::download::protocol;
use crate::download::store::{
    ByteRange, PartMet, PartState, RecoveredDownload, allocate_next_part_number, met_path_for_part,
    part_path_for_part, save_part_met, scan_recoverable_downloads,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot, watch};

pub type Result<T> = std::result::Result<T, DownloadError>;
const INFLIGHT_LEASE_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_RESERVE_BLOCKS_PER_CALL: usize = 128;

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
    pub recovered_on_start: usize,
    pub started_at: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadSummary {
    pub part_number: u16,
    pub file_name: String,
    pub file_size: u64,
    pub state: PartState,
    pub downloaded_bytes: u64,
    pub progress_pct: u8,
    pub missing_ranges: usize,
    pub inflight_ranges: usize,
    pub retry_count: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateDownloadRequest {
    pub file_name: String,
    pub file_size: u64,
    pub file_hash_md4_hex: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone)]
pub struct InboundPacket {
    pub opcode: u8,
    pub payload: Vec<u8>,
}

#[derive(Debug)]
pub enum DownloadCommand {
    Ping {
        reply: oneshot::Sender<()>,
    },
    RecoveredCount {
        reply: oneshot::Sender<usize>,
    },
    CreateDownload {
        req: CreateDownloadRequest,
        reply: oneshot::Sender<Result<DownloadSummary>>,
    },
    Pause {
        part_number: u16,
        reply: oneshot::Sender<Result<DownloadSummary>>,
    },
    Resume {
        part_number: u16,
        reply: oneshot::Sender<Result<DownloadSummary>>,
    },
    Cancel {
        part_number: u16,
        reply: oneshot::Sender<Result<DownloadSummary>>,
    },
    Delete {
        part_number: u16,
        reply: oneshot::Sender<Result<()>>,
    },
    List {
        reply: oneshot::Sender<Vec<DownloadSummary>>,
    },
    ReserveBlocks {
        part_number: u16,
        peer_id: String,
        max_blocks: usize,
        block_size: u64,
        reply: oneshot::Sender<Result<Vec<BlockRange>>>,
    },
    MarkBlockReceived {
        part_number: u16,
        peer_id: String,
        block: BlockRange,
        reply: oneshot::Sender<Result<DownloadSummary>>,
    },
    MarkBlockFailed {
        part_number: u16,
        peer_id: String,
        block: BlockRange,
        reason: String,
        reply: oneshot::Sender<Result<DownloadSummary>>,
    },
    PeerDisconnected {
        peer_id: String,
        reply: oneshot::Sender<Result<usize>>,
    },
    IngestInboundPacket {
        part_number: u16,
        peer_id: String,
        packet: InboundPacket,
        reply: oneshot::Sender<Result<DownloadSummary>>,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
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

    pub async fn recovered_count(&self) -> Result<usize> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(DownloadCommand::RecoveredCount { reply: tx })
            .await
            .map_err(|_| DownloadError::ChannelClosed)?;
        rx.await.map_err(|_| DownloadError::ChannelClosed)
    }

    pub async fn create_download(&self, req: CreateDownloadRequest) -> Result<DownloadSummary> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(DownloadCommand::CreateDownload { req, reply: tx })
            .await
            .map_err(|_| DownloadError::ChannelClosed)?;
        rx.await.map_err(|_| DownloadError::ChannelClosed)?
    }

    pub async fn pause(&self, part_number: u16) -> Result<DownloadSummary> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(DownloadCommand::Pause {
                part_number,
                reply: tx,
            })
            .await
            .map_err(|_| DownloadError::ChannelClosed)?;
        rx.await.map_err(|_| DownloadError::ChannelClosed)?
    }

    pub async fn resume(&self, part_number: u16) -> Result<DownloadSummary> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(DownloadCommand::Resume {
                part_number,
                reply: tx,
            })
            .await
            .map_err(|_| DownloadError::ChannelClosed)?;
        rx.await.map_err(|_| DownloadError::ChannelClosed)?
    }

    pub async fn cancel(&self, part_number: u16) -> Result<DownloadSummary> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(DownloadCommand::Cancel {
                part_number,
                reply: tx,
            })
            .await
            .map_err(|_| DownloadError::ChannelClosed)?;
        rx.await.map_err(|_| DownloadError::ChannelClosed)?
    }

    pub async fn delete(&self, part_number: u16) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(DownloadCommand::Delete {
                part_number,
                reply: tx,
            })
            .await
            .map_err(|_| DownloadError::ChannelClosed)?;
        rx.await.map_err(|_| DownloadError::ChannelClosed)?
    }

    pub async fn list(&self) -> Result<Vec<DownloadSummary>> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(DownloadCommand::List { reply: tx })
            .await
            .map_err(|_| DownloadError::ChannelClosed)?;
        rx.await.map_err(|_| DownloadError::ChannelClosed)
    }

    pub async fn reserve_blocks(
        &self,
        part_number: u16,
        max_blocks: usize,
        block_size: u64,
    ) -> Result<Vec<BlockRange>> {
        self.reserve_blocks_for_peer(part_number, "local".to_string(), max_blocks, block_size)
            .await
    }

    pub async fn reserve_blocks_for_peer(
        &self,
        part_number: u16,
        peer_id: String,
        max_blocks: usize,
        block_size: u64,
    ) -> Result<Vec<BlockRange>> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(DownloadCommand::ReserveBlocks {
                part_number,
                peer_id,
                max_blocks,
                block_size,
                reply: tx,
            })
            .await
            .map_err(|_| DownloadError::ChannelClosed)?;
        rx.await.map_err(|_| DownloadError::ChannelClosed)?
    }

    pub async fn mark_block_received(
        &self,
        part_number: u16,
        block: BlockRange,
    ) -> Result<DownloadSummary> {
        self.mark_block_received_by_peer(part_number, "local".to_string(), block)
            .await
    }

    pub async fn mark_block_received_by_peer(
        &self,
        part_number: u16,
        peer_id: String,
        block: BlockRange,
    ) -> Result<DownloadSummary> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(DownloadCommand::MarkBlockReceived {
                part_number,
                peer_id,
                block,
                reply: tx,
            })
            .await
            .map_err(|_| DownloadError::ChannelClosed)?;
        rx.await.map_err(|_| DownloadError::ChannelClosed)?
    }

    pub async fn mark_block_failed(
        &self,
        part_number: u16,
        block: BlockRange,
        reason: String,
    ) -> Result<DownloadSummary> {
        self.mark_block_failed_by_peer(part_number, "local".to_string(), block, reason)
            .await
    }

    pub async fn mark_block_failed_by_peer(
        &self,
        part_number: u16,
        peer_id: String,
        block: BlockRange,
        reason: String,
    ) -> Result<DownloadSummary> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(DownloadCommand::MarkBlockFailed {
                part_number,
                peer_id,
                block,
                reason,
                reply: tx,
            })
            .await
            .map_err(|_| DownloadError::ChannelClosed)?;
        rx.await.map_err(|_| DownloadError::ChannelClosed)?
    }

    pub async fn peer_disconnected(&self, peer_id: String) -> Result<usize> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(DownloadCommand::PeerDisconnected { peer_id, reply: tx })
            .await
            .map_err(|_| DownloadError::ChannelClosed)?;
        rx.await.map_err(|_| DownloadError::ChannelClosed)?
    }

    pub async fn ingest_inbound_packet(
        &self,
        part_number: u16,
        peer_id: String,
        packet: InboundPacket,
    ) -> Result<DownloadSummary> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(DownloadCommand::IngestInboundPacket {
                part_number,
                peer_id,
                packet,
                reply: tx,
            })
            .await
            .map_err(|_| DownloadError::ChannelClosed)?;
        rx.await.map_err(|_| DownloadError::ChannelClosed)?
    }

    #[cfg(test)]
    pub fn test_handle() -> Self {
        let (tx, mut rx) = mpsc::channel::<DownloadCommand>(64);
        tokio::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    DownloadCommand::Ping { reply } => {
                        let _ = reply.send(());
                    }
                    DownloadCommand::RecoveredCount { reply } => {
                        let _ = reply.send(0);
                    }
                    DownloadCommand::CreateDownload { reply, .. } => {
                        let _ = reply.send(Err(DownloadError::ChannelClosed));
                    }
                    DownloadCommand::Pause { reply, .. } => {
                        let _ = reply.send(Err(DownloadError::ChannelClosed));
                    }
                    DownloadCommand::Resume { reply, .. } => {
                        let _ = reply.send(Err(DownloadError::ChannelClosed));
                    }
                    DownloadCommand::Cancel { reply, .. } => {
                        let _ = reply.send(Err(DownloadError::ChannelClosed));
                    }
                    DownloadCommand::Delete { reply, .. } => {
                        let _ = reply.send(Err(DownloadError::ChannelClosed));
                    }
                    DownloadCommand::List { reply } => {
                        let _ = reply.send(Vec::new());
                    }
                    DownloadCommand::ReserveBlocks { reply, .. } => {
                        let _ = reply.send(Err(DownloadError::ChannelClosed));
                    }
                    DownloadCommand::MarkBlockReceived { reply, .. } => {
                        let _ = reply.send(Err(DownloadError::ChannelClosed));
                    }
                    DownloadCommand::MarkBlockFailed { reply, .. } => {
                        let _ = reply.send(Err(DownloadError::ChannelClosed));
                    }
                    DownloadCommand::PeerDisconnected { reply, .. } => {
                        let _ = reply.send(Err(DownloadError::ChannelClosed));
                    }
                    DownloadCommand::IngestInboundPacket { reply, .. } => {
                        let _ = reply.send(Err(DownloadError::ChannelClosed));
                    }
                    DownloadCommand::Shutdown { reply } => {
                        let _ = reply.send(());
                        break;
                    }
                }
            }
        });
        Self { tx }
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
    let recovered = scan_recoverable_downloads(&cfg.download_dir).await?;
    let recovered_count = recovered.len();
    let (tx, rx) = mpsc::channel(128);
    let started_at = Instant::now();
    let (status_tx, status_rx) = watch::channel(DownloadServiceStatus {
        running: true,
        queue_len: recovered_count,
        recovered_on_start: recovered_count,
        started_at,
    });
    let join = tokio::spawn(run_service(
        rx,
        status_tx,
        started_at,
        recovered,
        cfg.download_dir.clone(),
    ));
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
    recovered: Vec<RecoveredDownload>,
    download_dir: PathBuf,
) -> Result<()> {
    let mut downloads = std::collections::BTreeMap::<u16, ManagedDownload>::new();
    for r in recovered {
        let mut met = r.met;
        // In-flight ranges are not safe to resume blindly after restart. Put them
        // back into missing ranges and clear in-flight state.
        if !met.inflight_ranges.is_empty() {
            met.missing_ranges
                .extend(met.inflight_ranges.iter().copied());
            merge_ranges(&mut met.missing_ranges);
            met.inflight_ranges.clear();
            if met.state == PartState::Downloading {
                met.state = PartState::Queued;
            }
        }
        downloads.insert(
            met.part_number,
            ManagedDownload {
                met_path: r.met_path,
                part_path: r.part_path,
                met,
                leases: Vec::new(),
            },
        );
    }
    let recovered_on_start = downloads.len();
    let mut timeout_tick = tokio::time::interval(Duration::from_secs(1));
    timeout_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = timeout_tick.tick() => {
                let changed = process_timeouts(&mut downloads).await?;
                if changed {
                    publish_status(&status_tx, started_at, downloads.len(), recovered_on_start);
                }
            }
            cmd = rx.recv() => {
                let Some(cmd) = cmd else { break; };
                match cmd {
                    DownloadCommand::Ping { reply } => {
                        let _ = reply.send(());
                    }
                    DownloadCommand::RecoveredCount { reply } => {
                        let _ = reply.send(recovered_on_start);
                    }
                    DownloadCommand::CreateDownload { req, reply } => {
                        let result = create_download(&mut downloads, &download_dir, req).await;
                        publish_status(&status_tx, started_at, downloads.len(), recovered_on_start);
                        let _ = reply.send(result);
                    }
                    DownloadCommand::Pause { part_number, reply } => {
                        let result = set_state(
                            &mut downloads,
                            part_number,
                            PartState::Paused,
                            &[PartState::Queued, PartState::Downloading],
                        )
                        .await;
                        publish_status(&status_tx, started_at, downloads.len(), recovered_on_start);
                        let _ = reply.send(result);
                    }
                    DownloadCommand::Resume { part_number, reply } => {
                        let result = set_state(
                            &mut downloads,
                            part_number,
                            PartState::Queued,
                            &[PartState::Paused],
                        )
                        .await;
                        publish_status(&status_tx, started_at, downloads.len(), recovered_on_start);
                        let _ = reply.send(result);
                    }
                    DownloadCommand::Cancel { part_number, reply } => {
                        let result = set_state(
                            &mut downloads,
                            part_number,
                            PartState::Cancelled,
                            &[PartState::Queued, PartState::Paused, PartState::Downloading],
                        )
                        .await;
                        publish_status(&status_tx, started_at, downloads.len(), recovered_on_start);
                        let _ = reply.send(result);
                    }
                    DownloadCommand::Delete { part_number, reply } => {
                        let result = delete_download(&mut downloads, part_number).await;
                        publish_status(&status_tx, started_at, downloads.len(), recovered_on_start);
                        let _ = reply.send(result);
                    }
                    DownloadCommand::List { reply } => {
                        let _ = reply.send(list_summaries(&downloads));
                    }
                    DownloadCommand::ReserveBlocks {
                        part_number,
                        peer_id,
                        max_blocks,
                        block_size,
                        reply,
                    } => {
                        let result =
                            reserve_blocks(&mut downloads, part_number, peer_id, max_blocks, block_size).await;
                        publish_status(&status_tx, started_at, downloads.len(), recovered_on_start);
                        let _ = reply.send(result);
                    }
                    DownloadCommand::MarkBlockReceived {
                        part_number,
                        peer_id,
                        block,
                        reply,
                    } => {
                        let result = mark_block_received(&mut downloads, part_number, &peer_id, block).await;
                        publish_status(&status_tx, started_at, downloads.len(), recovered_on_start);
                        let _ = reply.send(result);
                    }
                    DownloadCommand::MarkBlockFailed {
                        part_number,
                        peer_id,
                        block,
                        reason,
                        reply,
                    } => {
                        let result = mark_block_failed(&mut downloads, part_number, &peer_id, block, reason).await;
                        publish_status(&status_tx, started_at, downloads.len(), recovered_on_start);
                        let _ = reply.send(result);
                    }
                    DownloadCommand::PeerDisconnected { peer_id, reply } => {
                        let result = peer_disconnected(&mut downloads, &peer_id).await;
                        publish_status(&status_tx, started_at, downloads.len(), recovered_on_start);
                        let _ = reply.send(result);
                    }
                    DownloadCommand::IngestInboundPacket { part_number, peer_id, packet, reply } => {
                        let result = ingest_inbound_packet(&mut downloads, part_number, &peer_id, packet).await;
                        publish_status(&status_tx, started_at, downloads.len(), recovered_on_start);
                        let _ = reply.send(result);
                    }
                    DownloadCommand::Shutdown { reply } => {
                        let _ = reply.send(());
                        let _ = status_tx.send(DownloadServiceStatus {
                            running: false,
                            queue_len: downloads.len(),
                            recovered_on_start,
                            started_at,
                        });
                        return Ok(());
                    }
                }
            }
        }
    }

    let _ = status_tx.send(DownloadServiceStatus {
        running: false,
        queue_len: downloads.len(),
        recovered_on_start,
        started_at,
    });
    Ok(())
}

#[derive(Debug, Clone)]
struct ManagedDownload {
    met_path: PathBuf,
    part_path: PathBuf,
    met: PartMet,
    leases: Vec<InflightLease>,
}

#[derive(Debug, Clone)]
struct InflightLease {
    range: BlockRange,
    peer_id: String,
    deadline: Instant,
}

fn list_summaries(
    downloads: &std::collections::BTreeMap<u16, ManagedDownload>,
) -> Vec<DownloadSummary> {
    downloads.values().map(summary_from_download).collect()
}

fn summary_from_download(d: &ManagedDownload) -> DownloadSummary {
    let progress = if d.met.file_size == 0 {
        0
    } else {
        ((d.met.downloaded_bytes.saturating_mul(100) / d.met.file_size).min(100)) as u8
    };
    DownloadSummary {
        part_number: d.met.part_number,
        file_name: d.met.file_name.clone(),
        file_size: d.met.file_size,
        state: d.met.state,
        downloaded_bytes: d.met.downloaded_bytes,
        progress_pct: progress,
        missing_ranges: d.met.missing_ranges.len(),
        inflight_ranges: d.met.inflight_ranges.len(),
        retry_count: d.met.retry_count,
        last_error: d.met.last_error.clone(),
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn create_download(
    downloads: &mut std::collections::BTreeMap<u16, ManagedDownload>,
    download_dir: &Path,
    req: CreateDownloadRequest,
) -> Result<DownloadSummary> {
    if req.file_name.trim().is_empty() {
        return Err(DownloadError::InvalidInput(
            "file_name must not be empty".to_string(),
        ));
    }
    if req.file_size == 0 {
        return Err(DownloadError::InvalidInput(
            "file_size must be > 0".to_string(),
        ));
    }

    let part_number = allocate_next_part_number(download_dir).await?;
    let met_path = met_path_for_part(download_dir, part_number);
    let part_path = part_path_for_part(download_dir, part_number);
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&part_path)
        .await
        .map_err(|source| DownloadStoreError::WriteFile {
            path: part_path.clone(),
            source,
        })?;
    tokio::io::AsyncWriteExt::flush(&mut file)
        .await
        .map_err(|source| DownloadStoreError::WriteFile {
            path: part_path.clone(),
            source,
        })?;
    file.set_len(req.file_size)
        .await
        .map_err(|source| DownloadStoreError::WriteFile {
            path: part_path.clone(),
            source,
        })?;

    let ts = now_secs();
    let met = PartMet {
        version: crate::download::store::PART_MET_VERSION,
        part_number,
        file_name: req.file_name,
        file_size: req.file_size,
        file_hash_md4_hex: req.file_hash_md4_hex,
        state: PartState::Queued,
        downloaded_bytes: 0,
        missing_ranges: vec![ByteRange {
            start: 0,
            end: req.file_size - 1,
        }],
        inflight_ranges: Vec::new(),
        retry_count: 0,
        last_error: None,
        created_unix_secs: ts,
        updated_unix_secs: ts,
    };
    save_part_met(&met_path, &met).await?;
    downloads.insert(
        part_number,
        ManagedDownload {
            met_path,
            part_path,
            met: met.clone(),
            leases: Vec::new(),
        },
    );
    Ok(summary_from_download(
        downloads.get(&part_number).expect("inserted"),
    ))
}

async fn set_state(
    downloads: &mut std::collections::BTreeMap<u16, ManagedDownload>,
    part_number: u16,
    next: PartState,
    allowed_prev: &[PartState],
) -> Result<DownloadSummary> {
    let d = downloads
        .get_mut(&part_number)
        .ok_or(DownloadError::NotFound(part_number))?;
    let prev = d.met.state;
    if !allowed_prev.contains(&prev) {
        return Err(DownloadError::InvalidTransition {
            part_number,
            from: prev,
            to: next,
        });
    }
    d.met.state = next;
    d.met.updated_unix_secs = now_secs();
    save_part_met(&d.met_path, &d.met).await?;
    Ok(summary_from_download(d))
}

async fn reserve_blocks(
    downloads: &mut std::collections::BTreeMap<u16, ManagedDownload>,
    part_number: u16,
    peer_id: String,
    max_blocks: usize,
    block_size: u64,
) -> Result<Vec<BlockRange>> {
    if max_blocks == 0 {
        return Ok(Vec::new());
    }
    if max_blocks > MAX_RESERVE_BLOCKS_PER_CALL {
        return Err(DownloadError::InvalidInput(format!(
            "max_blocks must be <= {MAX_RESERVE_BLOCKS_PER_CALL}"
        )));
    }
    if block_size == 0 {
        return Err(DownloadError::InvalidInput(
            "block_size must be > 0".to_string(),
        ));
    }

    let d = downloads
        .get_mut(&part_number)
        .ok_or(DownloadError::NotFound(part_number))?;
    match d.met.state {
        PartState::Queued | PartState::Downloading => {}
        other => {
            return Err(DownloadError::InvalidTransition {
                part_number,
                from: other,
                to: PartState::Downloading,
            });
        }
    }

    let mut out = Vec::new();
    let deadline = Instant::now() + INFLIGHT_LEASE_TIMEOUT;
    for _ in 0..max_blocks {
        let Some(first) = d.met.missing_ranges.first().copied() else {
            break;
        };
        let len = first.end.saturating_sub(first.start) + 1;
        let take = len.min(block_size);
        let block = BlockRange {
            start: first.start,
            end: first.start + take - 1,
        };
        d.met.missing_ranges = subtract_range(&d.met.missing_ranges, block.start, block.end);
        d.met.inflight_ranges.push(ByteRange {
            start: block.start,
            end: block.end,
        });
        d.leases.push(InflightLease {
            range: block,
            peer_id: peer_id.clone(),
            deadline,
        });
        out.push(block);
    }

    if !out.is_empty() {
        d.met.state = PartState::Downloading;
        d.met.updated_unix_secs = now_secs();
        d.met.last_error = None;
        save_part_met(&d.met_path, &d.met).await?;
    }
    Ok(out)
}

async fn mark_block_received(
    downloads: &mut std::collections::BTreeMap<u16, ManagedDownload>,
    part_number: u16,
    peer_id: &str,
    block: BlockRange,
) -> Result<DownloadSummary> {
    let d = downloads
        .get_mut(&part_number)
        .ok_or(DownloadError::NotFound(part_number))?;
    if !release_lease(d, peer_id, block) {
        return Err(DownloadError::InvalidInput(
            "block is not leased by this peer".to_string(),
        ));
    }
    remove_inflight(&mut d.met.inflight_ranges, block.start, block.end);

    d.met.downloaded_bytes = d
        .met
        .file_size
        .saturating_sub(total_missing(&d.met.missing_ranges));
    if d.met.missing_ranges.is_empty() && d.met.inflight_ranges.is_empty() {
        d.met.state = PartState::Completing;
    } else if d.met.state == PartState::Queued {
        d.met.state = PartState::Downloading;
    }
    d.met.updated_unix_secs = now_secs();
    d.met.last_error = None;
    save_part_met(&d.met_path, &d.met).await?;
    Ok(summary_from_download(d))
}

async fn mark_block_failed(
    downloads: &mut std::collections::BTreeMap<u16, ManagedDownload>,
    part_number: u16,
    peer_id: &str,
    block: BlockRange,
    reason: String,
) -> Result<DownloadSummary> {
    let d = downloads
        .get_mut(&part_number)
        .ok_or(DownloadError::NotFound(part_number))?;
    if !release_lease(d, peer_id, block) {
        return Err(DownloadError::InvalidInput(
            "block is not leased by this peer".to_string(),
        ));
    }
    remove_inflight(&mut d.met.inflight_ranges, block.start, block.end);
    d.met.missing_ranges.push(ByteRange {
        start: block.start,
        end: block.end,
    });
    merge_ranges(&mut d.met.missing_ranges);
    d.met.retry_count = d.met.retry_count.saturating_add(1);
    d.met.last_error = Some(reason);
    d.met.state = if d.met.state == PartState::Paused {
        PartState::Paused
    } else {
        PartState::Queued
    };
    d.met.downloaded_bytes = d
        .met
        .file_size
        .saturating_sub(total_missing(&d.met.missing_ranges));
    d.met.updated_unix_secs = now_secs();
    save_part_met(&d.met_path, &d.met).await?;
    Ok(summary_from_download(d))
}

async fn peer_disconnected(
    downloads: &mut std::collections::BTreeMap<u16, ManagedDownload>,
    peer_id: &str,
) -> Result<usize> {
    let mut reclaimed = 0usize;
    for d in downloads.values_mut() {
        let mut changed = false;
        let mut i = 0usize;
        while i < d.leases.len() {
            if d.leases[i].peer_id == peer_id {
                let lease = d.leases.remove(i);
                reclaimed = reclaimed.saturating_add(1);
                remove_inflight(
                    &mut d.met.inflight_ranges,
                    lease.range.start,
                    lease.range.end,
                );
                d.met.missing_ranges.push(ByteRange {
                    start: lease.range.start,
                    end: lease.range.end,
                });
                changed = true;
            } else {
                i += 1;
            }
        }
        if changed {
            merge_ranges(&mut d.met.missing_ranges);
            d.met.retry_count = d.met.retry_count.saturating_add(1);
            d.met.last_error = Some(format!("peer disconnected: {peer_id}"));
            if d.met.state == PartState::Downloading {
                d.met.state = PartState::Queued;
            }
            d.met.downloaded_bytes = d
                .met
                .file_size
                .saturating_sub(total_missing(&d.met.missing_ranges));
            d.met.updated_unix_secs = now_secs();
            save_part_met(&d.met_path, &d.met).await?;
        }
    }
    Ok(reclaimed)
}

async fn process_timeouts(
    downloads: &mut std::collections::BTreeMap<u16, ManagedDownload>,
) -> Result<bool> {
    let now = Instant::now();
    let mut changed_any = false;
    for d in downloads.values_mut() {
        let mut changed = false;
        let mut i = 0usize;
        while i < d.leases.len() {
            if d.leases[i].deadline <= now {
                let lease = d.leases.remove(i);
                remove_inflight(
                    &mut d.met.inflight_ranges,
                    lease.range.start,
                    lease.range.end,
                );
                d.met.missing_ranges.push(ByteRange {
                    start: lease.range.start,
                    end: lease.range.end,
                });
                changed = true;
            } else {
                i += 1;
            }
        }
        if changed {
            merge_ranges(&mut d.met.missing_ranges);
            d.met.retry_count = d.met.retry_count.saturating_add(1);
            d.met.last_error = Some("block timeout".to_string());
            if d.met.state == PartState::Downloading {
                d.met.state = PartState::Queued;
            }
            d.met.downloaded_bytes = d
                .met
                .file_size
                .saturating_sub(total_missing(&d.met.missing_ranges));
            d.met.updated_unix_secs = now_secs();
            save_part_met(&d.met_path, &d.met).await?;
            changed_any = true;
        }
    }
    Ok(changed_any)
}

async fn ingest_inbound_packet(
    downloads: &mut std::collections::BTreeMap<u16, ManagedDownload>,
    part_number: u16,
    peer_id: &str,
    packet: InboundPacket,
) -> Result<DownloadSummary> {
    let d = downloads
        .get(&part_number)
        .ok_or(DownloadError::NotFound(part_number))?;
    let expected_hash = parse_hex_hash(&d.met.file_hash_md4_hex)?;
    let part_path = d.part_path.clone();
    let file_size = d.met.file_size;

    let (block, block_data) = match packet.opcode {
        protocol::OP_SENDINGPART => {
            let p = protocol::decode_sendingpart_payload(&packet.payload).map_err(|e| {
                DownloadError::InvalidInput(format!("invalid sendingpart payload: {e}"))
            })?;
            if p.file_hash != expected_hash {
                return Err(DownloadError::InvalidInput(
                    "sendingpart file hash mismatch".to_string(),
                ));
            }
            (
                BlockRange {
                    start: p.start,
                    end: p.end_exclusive - 1,
                },
                p.data,
            )
        }
        protocol::OP_COMPRESSEDPART => {
            let p = protocol::decode_compressedpart_payload(&packet.payload).map_err(|e| {
                DownloadError::InvalidInput(format!("invalid compressedpart payload: {e}"))
            })?;
            if p.file_hash != expected_hash {
                return Err(DownloadError::InvalidInput(
                    "compressedpart file hash mismatch".to_string(),
                ));
            }
            if p.unpacked_len == 0 {
                return Err(DownloadError::InvalidInput(
                    "compressedpart unpacked_len must be > 0".to_string(),
                ));
            }
            let decompressed =
                crate::kad::packed::inflate_zlib(&p.compressed_data, p.unpacked_len as usize)
                    .map_err(|e| {
                        DownloadError::InvalidInput(format!(
                            "invalid compressedpart zlib payload: {e}"
                        ))
                    })?;
            if decompressed.len() != p.unpacked_len as usize {
                return Err(DownloadError::InvalidInput(format!(
                    "compressedpart unpacked_len mismatch: declared={}, actual={}",
                    p.unpacked_len,
                    decompressed.len()
                )));
            }
            let end = p
                .start
                .checked_add(u64::from(p.unpacked_len))
                .and_then(|v| v.checked_sub(1))
                .ok_or_else(|| {
                    DownloadError::InvalidInput("compressedpart range overflow".to_string())
                })?;
            (
                BlockRange {
                    start: p.start,
                    end,
                },
                decompressed,
            )
        }
        other => {
            return Err(DownloadError::InvalidInput(format!(
                "unsupported inbound opcode 0x{other:02x}"
            )));
        }
    };
    if block.end >= file_size {
        return Err(DownloadError::InvalidInput(format!(
            "inbound block out of file range: end={} file_size={}",
            block.end, file_size
        )));
    }
    persist_part_block(&part_path, block.start, &block_data).await?;
    mark_block_received(downloads, part_number, peer_id, block).await
}

async fn persist_part_block(path: &Path, start: u64, data: &[u8]) -> Result<()> {
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .await
        .map_err(|source| DownloadStoreError::WriteFile {
            path: path.to_path_buf(),
            source,
        })?;
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|source| DownloadStoreError::WriteFile {
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(data)
        .await
        .map_err(|source| DownloadStoreError::WriteFile {
            path: path.to_path_buf(),
            source,
        })?;
    file.flush()
        .await
        .map_err(|source| DownloadStoreError::WriteFile {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(())
}

async fn delete_download(
    downloads: &mut std::collections::BTreeMap<u16, ManagedDownload>,
    part_number: u16,
) -> Result<()> {
    let (met_path, part_path) = {
        let d = downloads
            .get(&part_number)
            .ok_or(DownloadError::NotFound(part_number))?;
        (d.met_path.clone(), d.part_path.clone())
    };
    if met_path.exists() {
        tokio::fs::remove_file(&met_path).await.map_err(|source| {
            DownloadStoreError::WriteFile {
                path: met_path.clone(),
                source,
            }
        })?;
    }
    let bak = {
        let mut p = met_path.as_os_str().to_os_string();
        p.push(".bak");
        PathBuf::from(p)
    };
    if bak.exists() {
        tokio::fs::remove_file(&bak)
            .await
            .map_err(|source| DownloadStoreError::WriteFile { path: bak, source })?;
    }
    if part_path.exists() {
        tokio::fs::remove_file(&part_path).await.map_err(|source| {
            DownloadStoreError::WriteFile {
                path: part_path.clone(),
                source,
            }
        })?;
    }
    let _ = downloads.remove(&part_number);
    Ok(())
}

fn publish_status(
    status_tx: &watch::Sender<DownloadServiceStatus>,
    started_at: Instant,
    queue_len: usize,
    recovered_on_start: usize,
) {
    let _ = status_tx.send(DownloadServiceStatus {
        running: true,
        queue_len,
        recovered_on_start,
        started_at,
    });
}

fn total_missing(ranges: &[ByteRange]) -> u64 {
    ranges
        .iter()
        .map(|r| r.end.saturating_sub(r.start) + 1)
        .sum()
}

fn merge_ranges(ranges: &mut Vec<ByteRange>) {
    if ranges.is_empty() {
        return;
    }
    ranges.sort_by_key(|r| (r.start, r.end));
    let mut merged = Vec::with_capacity(ranges.len());
    let mut cur = ranges[0];
    for r in ranges.iter().copied().skip(1) {
        if r.start <= cur.end.saturating_add(1) {
            cur.end = cur.end.max(r.end);
        } else {
            merged.push(cur);
            cur = r;
        }
    }
    merged.push(cur);
    *ranges = merged;
}

fn subtract_range(ranges: &[ByteRange], start: u64, end: u64) -> Vec<ByteRange> {
    let mut out = Vec::new();
    for r in ranges.iter().copied() {
        if end < r.start || start > r.end {
            out.push(r);
            continue;
        }
        if start > r.start {
            out.push(ByteRange {
                start: r.start,
                end: start - 1,
            });
        }
        if end < r.end {
            out.push(ByteRange {
                start: end + 1,
                end: r.end,
            });
        }
    }
    out
}

fn remove_inflight(inflight: &mut Vec<ByteRange>, start: u64, end: u64) {
    if let Some(idx) = inflight
        .iter()
        .position(|r| r.start == start && r.end == end)
    {
        inflight.remove(idx);
    }
}

fn release_lease(d: &mut ManagedDownload, peer_id: &str, block: BlockRange) -> bool {
    if let Some(idx) = d
        .leases
        .iter()
        .position(|l| l.peer_id == peer_id && l.range == block)
    {
        d.leases.remove(idx);
        true
    } else {
        false
    }
}

fn parse_hex_hash(s: &str) -> Result<[u8; 16]> {
    if s.len() != 32 {
        return Err(DownloadError::InvalidInput(
            "file hash must be 32 hex chars".to_string(),
        ));
    }
    let mut out = [0u8; 16];
    for (idx, chunk) in s.as_bytes().chunks_exact(2).enumerate() {
        let pair = std::str::from_utf8(chunk)
            .map_err(|_| DownloadError::InvalidInput("file hash must be valid hex".to_string()))?;
        out[idx] = u8::from_str_radix(pair, 16)
            .map_err(|_| DownloadError::InvalidInput("file hash must be valid hex".to_string()))?;
    }
    Ok(out)
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
        assert_eq!(status_rx.borrow().queue_len, 0);
        assert_eq!(handle.recovered_count().await.expect("recovered count"), 0);

        handle.shutdown().await.expect("shutdown");
        status_rx.changed().await.expect("status changed");
        assert!(!status_rx.borrow().running);

        join.await.expect("join").expect("service");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn service_recovers_existing_part_met_entries_on_start() {
        let root = temp_dir("recover");
        tokio::fs::create_dir_all(root.join("download"))
            .await
            .expect("mkdir download");

        let m1 = crate::download::store::PartMet {
            version: crate::download::store::PART_MET_VERSION,
            part_number: 1,
            file_name: "a.bin".to_string(),
            file_size: 10,
            file_hash_md4_hex: "0123456789abcdef0123456789abcdef".to_string(),
            state: crate::download::store::PartState::Queued,
            downloaded_bytes: 0,
            missing_ranges: vec![ByteRange { start: 0, end: 9 }],
            inflight_ranges: Vec::new(),
            retry_count: 0,
            last_error: None,
            created_unix_secs: 1,
            updated_unix_secs: 1,
        };
        let m2 = crate::download::store::PartMet {
            version: crate::download::store::PART_MET_VERSION,
            part_number: 2,
            file_name: "b.bin".to_string(),
            file_size: 20,
            file_hash_md4_hex: "fedcba9876543210fedcba9876543210".to_string(),
            state: crate::download::store::PartState::Paused,
            downloaded_bytes: 5,
            missing_ranges: vec![ByteRange { start: 5, end: 19 }],
            inflight_ranges: Vec::new(),
            retry_count: 0,
            last_error: None,
            created_unix_secs: 1,
            updated_unix_secs: 2,
        };

        crate::download::store::save_part_met(&root.join("download/001.part.met"), &m1)
            .await
            .expect("save m1");
        crate::download::store::save_part_met(&root.join("download/002.part.met"), &m2)
            .await
            .expect("save m2");

        let cfg = DownloadServiceConfig::from_data_dir(&root);
        let (handle, status_rx, join) = start_service(cfg).await.expect("start");
        assert_eq!(status_rx.borrow().queue_len, 2);
        assert_eq!(status_rx.borrow().recovered_on_start, 2);
        assert_eq!(handle.recovered_count().await.expect("count"), 2);

        handle.shutdown().await.expect("shutdown");
        join.await.expect("join").expect("service");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn service_create_pause_resume_cancel_delete_list_flow() {
        let root = temp_dir("flow2");
        let cfg = DownloadServiceConfig::from_data_dir(&root);
        let (handle, status_rx, join) = start_service(cfg).await.expect("start");

        let created = handle
            .create_download(CreateDownloadRequest {
                file_name: "movie.avi".to_string(),
                file_size: 1234,
                file_hash_md4_hex: "0123456789abcdef0123456789abcdef".to_string(),
            })
            .await
            .expect("create");
        assert_eq!(created.part_number, 1);
        assert_eq!(created.state, PartState::Queued);
        assert_eq!(created.progress_pct, 0);
        assert_eq!(status_rx.borrow().queue_len, 1);

        let paused = handle.pause(1).await.expect("pause");
        assert_eq!(paused.state, PartState::Paused);

        let resumed = handle.resume(1).await.expect("resume");
        assert_eq!(resumed.state, PartState::Queued);

        let cancelled = handle.cancel(1).await.expect("cancel");
        assert_eq!(cancelled.state, PartState::Cancelled);

        let listed = handle.list().await.expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].state, PartState::Cancelled);

        handle.delete(1).await.expect("delete");
        let listed2 = handle.list().await.expect("list2");
        assert!(listed2.is_empty());

        handle.shutdown().await.expect("shutdown");
        join.await.expect("join").expect("service");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn service_recovers_persisted_state_after_restart() {
        let root = temp_dir("restart");
        let cfg = DownloadServiceConfig::from_data_dir(&root);

        let (handle1, _status1, join1) = start_service(cfg.clone()).await.expect("start1");
        let d = handle1
            .create_download(CreateDownloadRequest {
                file_name: "resume.iso".to_string(),
                file_size: 4096,
                file_hash_md4_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            })
            .await
            .expect("create");
        handle1.pause(d.part_number).await.expect("pause");
        handle1.shutdown().await.expect("shutdown1");
        join1.await.expect("join1").expect("svc1");

        let (handle2, _status2, join2) = start_service(cfg).await.expect("start2");
        let list = handle2.list().await.expect("list2");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].part_number, d.part_number);
        assert_eq!(list[0].state, PartState::Paused);
        handle2.shutdown().await.expect("shutdown2");
        join2.await.expect("join2").expect("svc2");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn transfer_reserve_fail_retry_and_receive_updates_progress() {
        let root = temp_dir("transfer");
        let cfg = DownloadServiceConfig::from_data_dir(&root);
        let (handle, _status, join) = start_service(cfg).await.expect("start");
        let d = handle
            .create_download(CreateDownloadRequest {
                file_name: "sample.bin".to_string(),
                file_size: 1000,
                file_hash_md4_hex: "00112233445566778899aabbccddeeff".to_string(),
            })
            .await
            .expect("create");

        let blocks = handle
            .reserve_blocks(d.part_number, 2, 200)
            .await
            .expect("reserve");
        assert_eq!(blocks.len(), 2);

        let failed = handle
            .mark_block_failed(d.part_number, blocks[0], "timeout".to_string())
            .await
            .expect("failed");
        assert_eq!(failed.retry_count, 1);
        assert_eq!(failed.state, PartState::Queued);
        assert_eq!(failed.last_error.as_deref(), Some("timeout"));

        let again = handle
            .reserve_blocks(d.part_number, 1, 200)
            .await
            .expect("reserve again");
        assert_eq!(again.len(), 1);

        let got = handle
            .mark_block_received(d.part_number, again[0])
            .await
            .expect("received");
        assert!(got.progress_pct > 0);
        assert_eq!(got.inflight_ranges, 1);

        handle.shutdown().await.expect("shutdown");
        join.await.expect("join").expect("svc");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn reserve_blocks_rejects_excessive_max_blocks() {
        let root = temp_dir("reserve-cap");
        let cfg = DownloadServiceConfig::from_data_dir(&root);
        let (handle, _status, join) = start_service(cfg).await.expect("start");
        let d = handle
            .create_download(CreateDownloadRequest {
                file_name: "cap.bin".to_string(),
                file_size: 1000,
                file_hash_md4_hex: "00112233445566778899aabbccddeeff".to_string(),
            })
            .await
            .expect("create");

        let err = handle
            .reserve_blocks(d.part_number, MAX_RESERVE_BLOCKS_PER_CALL + 1, 200)
            .await
            .expect_err("must reject too many blocks");
        assert!(matches!(err, DownloadError::InvalidInput(_)));

        handle.shutdown().await.expect("shutdown");
        join.await.expect("join").expect("svc");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn restart_reclaims_inflight_back_into_missing() {
        let root = temp_dir("reclaim");
        let cfg = DownloadServiceConfig::from_data_dir(&root);

        let (handle1, _status1, join1) = start_service(cfg.clone()).await.expect("start1");
        let d = handle1
            .create_download(CreateDownloadRequest {
                file_name: "reclaim.bin".to_string(),
                file_size: 500,
                file_hash_md4_hex: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            })
            .await
            .expect("create");
        let _ = handle1
            .reserve_blocks(d.part_number, 1, 100)
            .await
            .expect("reserve");
        handle1.shutdown().await.expect("shutdown1");
        join1.await.expect("join1").expect("svc1");

        let (handle2, _status2, join2) = start_service(cfg).await.expect("start2");
        let list = handle2.list().await.expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].inflight_ranges, 0);
        assert_eq!(list[0].state, PartState::Queued);

        handle2.shutdown().await.expect("shutdown2");
        join2.await.expect("join2").expect("svc2");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn peer_disconnected_reclaims_only_that_peers_leases() {
        let root = temp_dir("peer-drop");
        let cfg = DownloadServiceConfig::from_data_dir(&root);
        let (handle, _status, join) = start_service(cfg).await.expect("start");
        let d = handle
            .create_download(CreateDownloadRequest {
                file_name: "peer.bin".to_string(),
                file_size: 600,
                file_hash_md4_hex: "11111111111111111111111111111111".to_string(),
            })
            .await
            .expect("create");

        let a = handle
            .reserve_blocks_for_peer(d.part_number, "peer-a".to_string(), 1, 100)
            .await
            .expect("reserve a");
        let b = handle
            .reserve_blocks_for_peer(d.part_number, "peer-b".to_string(), 1, 100)
            .await
            .expect("reserve b");
        assert_eq!(a.len(), 1);
        assert_eq!(b.len(), 1);

        let reclaimed = handle
            .peer_disconnected("peer-a".to_string())
            .await
            .expect("disconnect");
        assert_eq!(reclaimed, 1);

        let list = handle.list().await.expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].inflight_ranges, 1);
        assert_eq!(list[0].retry_count, 1);

        handle.shutdown().await.expect("shutdown");
        join.await.expect("join").expect("svc");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn ingest_sendingpart_marks_reserved_block_received() {
        let root = temp_dir("ingest");
        let cfg = DownloadServiceConfig::from_data_dir(&root);
        let (handle, _status, join) = start_service(cfg).await.expect("start");
        let d = handle
            .create_download(CreateDownloadRequest {
                file_name: "ingest.bin".to_string(),
                file_size: 256,
                file_hash_md4_hex: "0123456789abcdef0123456789abcdef".to_string(),
            })
            .await
            .expect("create");
        let reserved = handle
            .reserve_blocks_for_peer(d.part_number, "peer-a".to_string(), 1, 64)
            .await
            .expect("reserve");
        assert_eq!(reserved.len(), 1);
        let block = reserved[0];

        let mut payload = Vec::new();
        payload.extend_from_slice(&hex_hash("0123456789abcdef0123456789abcdef"));
        payload.extend_from_slice(&block.start.to_le_bytes());
        payload.extend_from_slice(&(block.end + 1).to_le_bytes());
        payload.extend_from_slice(&vec![0xAA; (block.end - block.start + 1) as usize]);

        let got = handle
            .ingest_inbound_packet(
                d.part_number,
                "peer-a".to_string(),
                InboundPacket {
                    opcode: protocol::OP_SENDINGPART,
                    payload,
                },
            )
            .await
            .expect("ingest");
        assert_eq!(got.inflight_ranges, 0);
        assert!(got.progress_pct > 0);

        handle.shutdown().await.expect("shutdown");
        join.await.expect("join").expect("svc");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn ingest_compressedpart_decompresses_persists_then_marks_received() {
        let root = temp_dir("ingest-compressed");
        let cfg = DownloadServiceConfig::from_data_dir(&root);
        let (handle, _status, join) = start_service(cfg.clone()).await.expect("start");
        let d = handle
            .create_download(CreateDownloadRequest {
                file_name: "ingest-compressed.bin".to_string(),
                file_size: 5,
                file_hash_md4_hex: "0123456789abcdef0123456789abcdef".to_string(),
            })
            .await
            .expect("create");
        let reserved = handle
            .reserve_blocks_for_peer(d.part_number, "peer-a".to_string(), 1, 5)
            .await
            .expect("reserve");
        assert_eq!(reserved.len(), 1);
        assert_eq!(reserved[0], BlockRange { start: 0, end: 4 });

        // zlib-compressed "hello"
        let compressed = vec![
            0x78, 0x9c, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x07, 0x00, 0x06, 0x2c, 0x02, 0x15,
        ];
        let mut payload = Vec::new();
        payload.extend_from_slice(&hex_hash("0123456789abcdef0123456789abcdef"));
        payload.extend_from_slice(&0u64.to_le_bytes());
        payload.extend_from_slice(&5u32.to_le_bytes());
        payload.extend_from_slice(&compressed);

        let got = handle
            .ingest_inbound_packet(
                d.part_number,
                "peer-a".to_string(),
                InboundPacket {
                    opcode: protocol::OP_COMPRESSEDPART,
                    payload,
                },
            )
            .await
            .expect("ingest");
        assert_eq!(got.inflight_ranges, 0);
        assert_eq!(got.progress_pct, 100);

        let part_path = part_path_for_part(&cfg.download_dir, d.part_number);
        let bytes = tokio::fs::read(&part_path).await.expect("read part");
        assert_eq!(&bytes[..5], b"hello");

        handle.shutdown().await.expect("shutdown");
        join.await.expect("join").expect("svc");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn ingest_compressedpart_rejects_invalid_zlib_and_keeps_inflight() {
        let root = temp_dir("ingest-compressed-invalid");
        let cfg = DownloadServiceConfig::from_data_dir(&root);
        let (handle, _status, join) = start_service(cfg).await.expect("start");
        let d = handle
            .create_download(CreateDownloadRequest {
                file_name: "ingest-compressed-invalid.bin".to_string(),
                file_size: 5,
                file_hash_md4_hex: "0123456789abcdef0123456789abcdef".to_string(),
            })
            .await
            .expect("create");
        let reserved = handle
            .reserve_blocks_for_peer(d.part_number, "peer-a".to_string(), 1, 5)
            .await
            .expect("reserve");
        assert_eq!(reserved, vec![BlockRange { start: 0, end: 4 }]);

        let mut payload = Vec::new();
        payload.extend_from_slice(&hex_hash("0123456789abcdef0123456789abcdef"));
        payload.extend_from_slice(&0u64.to_le_bytes());
        payload.extend_from_slice(&5u32.to_le_bytes());
        payload.extend_from_slice(b"not-zlib");

        let err = handle
            .ingest_inbound_packet(
                d.part_number,
                "peer-a".to_string(),
                InboundPacket {
                    opcode: protocol::OP_COMPRESSEDPART,
                    payload,
                },
            )
            .await
            .expect_err("invalid zlib must fail");
        assert!(matches!(err, DownloadError::InvalidInput(_)));

        let list = handle.list().await.expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].inflight_ranges, 1);
        assert_eq!(list[0].downloaded_bytes, 0);

        handle.shutdown().await.expect("shutdown");
        join.await.expect("join").expect("svc");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn delete_download_keeps_state_when_disk_delete_fails() {
        let root = temp_dir("delete-fail");
        tokio::fs::create_dir_all(&root).await.expect("mkdir");

        let met_path = root.join("001.part.met");
        tokio::fs::create_dir_all(&met_path)
            .await
            .expect("mkdir met path");
        let part_path = root.join("001.part");
        tokio::fs::write(&part_path, b"test")
            .await
            .expect("write part");

        let met = PartMet {
            version: crate::download::store::PART_MET_VERSION,
            part_number: 1,
            file_name: "x.bin".to_string(),
            file_size: 4,
            file_hash_md4_hex: "0123456789abcdef0123456789abcdef".to_string(),
            state: PartState::Queued,
            downloaded_bytes: 0,
            missing_ranges: vec![ByteRange { start: 0, end: 3 }],
            inflight_ranges: Vec::new(),
            retry_count: 0,
            last_error: None,
            created_unix_secs: 1,
            updated_unix_secs: 1,
        };

        let mut downloads = std::collections::BTreeMap::new();
        downloads.insert(
            1,
            ManagedDownload {
                met_path: met_path.clone(),
                part_path: part_path.clone(),
                met,
                leases: Vec::new(),
            },
        );

        let err = delete_download(&mut downloads, 1)
            .await
            .expect_err("delete should fail");
        assert!(matches!(err, DownloadError::Store(_)));
        assert!(downloads.contains_key(&1));

        let _ = std::fs::remove_dir_all(&root);
    }

    fn hex_hash(input: &str) -> [u8; 16] {
        let mut out = [0u8; 16];
        for (idx, chunk) in input.as_bytes().chunks_exact(2).enumerate() {
            let pair = std::str::from_utf8(chunk).expect("utf8");
            out[idx] = u8::from_str_radix(pair, 16).expect("hex");
        }
        out
    }
}
