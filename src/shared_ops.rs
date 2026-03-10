use crate::{
    config::Config,
    kad::service::KadServiceCommand,
    publish::SharedPublishTracker,
    share::{self, SharedLibrary},
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    path::Path,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Mutex, RwLock, mpsc};

const REPUBLISH_SOURCES_COOLDOWN_SECS: u64 = 300;
const REPUBLISH_KEYWORDS_COOLDOWN_SECS: u64 = 900;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SharedActionKind {
    Reindex,
    RepublishSources,
    RepublishKeywords,
}

impl SharedActionKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Reindex => "reindex",
            Self::RepublishSources => "republish_sources",
            Self::RepublishKeywords => "republish_keywords",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SharedActionRejectReason {
    AlreadyRunning,
    CooldownActive,
}

#[derive(Debug, Clone, Serialize)]
pub struct SharedActionStatus {
    pub action: String,
    pub state: String,
    pub started_unix_secs: Option<u64>,
    pub finished_unix_secs: Option<u64>,
    pub cooldown_until_unix_secs: Option<u64>,
    pub items_total: usize,
    pub queued_total: usize,
    pub failed_total: usize,
    pub library_files_total: Option<usize>,
    pub reused_entries: Option<usize>,
    pub hashed_entries: Option<usize>,
    pub last_error: Option<String>,
}

impl SharedActionStatus {
    fn idle(kind: SharedActionKind) -> Self {
        Self {
            action: kind.as_str().to_string(),
            state: "idle".to_string(),
            started_unix_secs: None,
            finished_unix_secs: None,
            cooldown_until_unix_secs: None,
            items_total: 0,
            queued_total: 0,
            failed_total: 0,
            library_files_total: None,
            reused_entries: None,
            hashed_entries: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SharedActionStartResponse {
    pub started: bool,
    pub reason: Option<SharedActionRejectReason>,
    pub status: SharedActionStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct SharedActionsSnapshot {
    pub actions: Vec<SharedActionStatus>,
}

#[derive(Debug, Clone)]
pub struct SharedOpsManager {
    library: Arc<RwLock<SharedLibrary>>,
    config: Arc<tokio::sync::Mutex<Config>>,
    publish_tracker: Arc<SharedPublishTracker>,
    kad_cmd_tx: mpsc::Sender<KadServiceCommand>,
    statuses: Arc<Mutex<BTreeMap<SharedActionKind, SharedActionStatus>>>,
}

impl SharedOpsManager {
    pub fn new(
        library: Arc<RwLock<SharedLibrary>>,
        config: Arc<tokio::sync::Mutex<Config>>,
        publish_tracker: Arc<SharedPublishTracker>,
        kad_cmd_tx: mpsc::Sender<KadServiceCommand>,
    ) -> Self {
        let statuses = BTreeMap::from([
            (
                SharedActionKind::Reindex,
                SharedActionStatus::idle(SharedActionKind::Reindex),
            ),
            (
                SharedActionKind::RepublishSources,
                SharedActionStatus::idle(SharedActionKind::RepublishSources),
            ),
            (
                SharedActionKind::RepublishKeywords,
                SharedActionStatus::idle(SharedActionKind::RepublishKeywords),
            ),
        ]);
        Self {
            library,
            config,
            publish_tracker,
            kad_cmd_tx,
            statuses: Arc::new(Mutex::new(statuses)),
        }
    }

    pub async fn library_snapshot(&self) -> SharedLibrary {
        self.library.read().await.clone()
    }

    pub async fn action_snapshot(&self) -> SharedActionsSnapshot {
        let statuses = self.statuses.lock().await;
        SharedActionsSnapshot {
            actions: statuses.values().cloned().collect(),
        }
    }

    pub async fn start_reindex(&self) -> SharedActionStartResponse {
        self.start_action(SharedActionKind::Reindex).await
    }

    pub async fn start_republish_sources(&self) -> SharedActionStartResponse {
        self.start_action(SharedActionKind::RepublishSources).await
    }

    pub async fn start_republish_keywords(&self) -> SharedActionStartResponse {
        self.start_action(SharedActionKind::RepublishKeywords).await
    }

    async fn start_action(&self, kind: SharedActionKind) -> SharedActionStartResponse {
        let now = now_unix_secs();
        {
            let mut statuses = self.statuses.lock().await;
            let status = statuses
                .entry(kind)
                .or_insert_with(|| SharedActionStatus::idle(kind));
            if status.state == "running" {
                return SharedActionStartResponse {
                    started: false,
                    reason: Some(SharedActionRejectReason::AlreadyRunning),
                    status: status.clone(),
                };
            }
            if status
                .cooldown_until_unix_secs
                .is_some_and(|cooldown_until| now.is_some_and(|current| current < cooldown_until))
            {
                return SharedActionStartResponse {
                    started: false,
                    reason: Some(SharedActionRejectReason::CooldownActive),
                    status: status.clone(),
                };
            }
            *status = SharedActionStatus {
                action: kind.as_str().to_string(),
                state: "running".to_string(),
                started_unix_secs: now,
                finished_unix_secs: None,
                cooldown_until_unix_secs: None,
                items_total: 0,
                queued_total: 0,
                failed_total: 0,
                library_files_total: None,
                reused_entries: None,
                hashed_entries: None,
                last_error: None,
            };
        }

        let manager = self.clone();
        tokio::spawn(async move {
            manager.run_action(kind).await;
        });

        let statuses = self.statuses.lock().await;
        SharedActionStartResponse {
            started: true,
            reason: None,
            status: statuses
                .get(&kind)
                .cloned()
                .unwrap_or_else(|| SharedActionStatus::idle(kind)),
        }
    }

    async fn run_action(&self, kind: SharedActionKind) {
        let result = match kind {
            SharedActionKind::Reindex => self.run_reindex().await,
            SharedActionKind::RepublishSources => self.run_republish_sources().await,
            SharedActionKind::RepublishKeywords => self.run_republish_keywords().await,
        };
        let mut statuses = self.statuses.lock().await;
        let status = statuses
            .entry(kind)
            .or_insert_with(|| SharedActionStatus::idle(kind));
        let started_unix_secs = status.started_unix_secs;
        match result {
            Ok(mut report) => {
                report.started_unix_secs = started_unix_secs;
                *status = report;
            }
            Err(err) => {
                *status = SharedActionStatus {
                    action: kind.as_str().to_string(),
                    state: "failed".to_string(),
                    started_unix_secs,
                    finished_unix_secs: now_unix_secs(),
                    cooldown_until_unix_secs: None,
                    items_total: 0,
                    queued_total: 0,
                    failed_total: 0,
                    library_files_total: None,
                    reused_entries: None,
                    hashed_entries: None,
                    last_error: Some(err),
                };
            }
        }
    }

    async fn run_reindex(&self) -> std::result::Result<SharedActionStatus, String> {
        let config = self.config.lock().await.clone();
        let data_dir = Path::new(&config.general.data_dir);
        let roots = share::canonicalize_share_roots(&config.sharing.share_roots, data_dir)
            .map_err(|err| err.to_string())?;
        let cache_path = data_dir.join("shared_library.json");
        let build = share::load_or_rebuild_shared_library(&roots, &cache_path)
            .await
            .map_err(|err| err.to_string())?;
        let library_files_total = build.library.len();
        *self.library.write().await = build.library;
        Ok(SharedActionStatus {
            action: SharedActionKind::Reindex.as_str().to_string(),
            state: "succeeded".to_string(),
            started_unix_secs: None,
            finished_unix_secs: now_unix_secs(),
            cooldown_until_unix_secs: None,
            items_total: roots.len(),
            queued_total: 0,
            failed_total: 0,
            library_files_total: Some(library_files_total),
            reused_entries: Some(build.stats.reused_entries),
            hashed_entries: Some(build.stats.hashed_entries),
            last_error: None,
        })
    }

    async fn run_republish_sources(&self) -> std::result::Result<SharedActionStatus, String> {
        self.ensure_kad_service_enabled().await?;
        let library = self.library_snapshot().await;
        let (queued_total, failed_total) =
            queue_source_publishes(library.files(), &self.kad_cmd_tx, &self.publish_tracker).await;
        Ok(SharedActionStatus {
            action: SharedActionKind::RepublishSources.as_str().to_string(),
            state: if failed_total == 0 {
                "succeeded"
            } else {
                "failed"
            }
            .to_string(),
            started_unix_secs: None,
            finished_unix_secs: now_unix_secs(),
            cooldown_until_unix_secs: cooldown_until(REPUBLISH_SOURCES_COOLDOWN_SECS),
            items_total: library.len(),
            queued_total,
            failed_total,
            library_files_total: Some(library.len()),
            reused_entries: None,
            hashed_entries: None,
            last_error: (failed_total > 0)
                .then(|| "one or more source publishes failed to queue".to_string()),
        })
    }

    async fn run_republish_keywords(&self) -> std::result::Result<SharedActionStatus, String> {
        self.ensure_kad_service_enabled().await?;
        let library = self.library_snapshot().await;
        let (items_total, queued_total, failed_total) =
            queue_keyword_publishes(library.files(), &self.kad_cmd_tx, &self.publish_tracker).await;
        Ok(SharedActionStatus {
            action: SharedActionKind::RepublishKeywords.as_str().to_string(),
            state: if failed_total == 0 {
                "succeeded"
            } else {
                "failed"
            }
            .to_string(),
            started_unix_secs: None,
            finished_unix_secs: now_unix_secs(),
            cooldown_until_unix_secs: cooldown_until(REPUBLISH_KEYWORDS_COOLDOWN_SECS),
            items_total,
            queued_total,
            failed_total,
            library_files_total: Some(library.len()),
            reused_entries: None,
            hashed_entries: None,
            last_error: (failed_total > 0)
                .then(|| "one or more keyword publishes failed to queue".to_string()),
        })
    }

    async fn ensure_kad_service_enabled(&self) -> std::result::Result<(), String> {
        let config = self.config.lock().await;
        if config.kad.service_enabled {
            Ok(())
        } else {
            Err("KAD service is disabled".to_string())
        }
    }
}

pub async fn queue_source_publishes(
    files: &[share::SharedLibraryFile],
    kad_cmd_tx: &mpsc::Sender<KadServiceCommand>,
    publish_tracker: &SharedPublishTracker,
) -> (usize, usize) {
    let mut queued_total = 0usize;
    let mut failed_total = 0usize;
    for file in files {
        tracing::info!(
            path = %file.relative_path.display(),
            hash = %file.file_hash_md4_hex,
            size = file.file_size,
            "queueing shared file source publish"
        );
        if kad_cmd_tx
            .send(KadServiceCommand::PublishSource {
                file: file.file_id,
                file_size: file.file_size,
            })
            .await
            .is_err()
        {
            failed_total += 1;
            publish_tracker.note_source_queue_failed(&file.file_hash_md4_hex);
            tracing::warn!(
                path = %file.relative_path.display(),
                hash = %file.file_hash_md4_hex,
                "failed to queue shared file source publish"
            );
            continue;
        }
        queued_total += 1;
        publish_tracker.note_source_queued(&file.file_hash_md4_hex);
    }
    (queued_total, failed_total)
}

pub async fn queue_keyword_publishes(
    files: &[share::SharedLibraryFile],
    kad_cmd_tx: &mpsc::Sender<KadServiceCommand>,
    publish_tracker: &SharedPublishTracker,
) -> (usize, usize, usize) {
    let mut items_total = 0usize;
    let mut queued_total = 0usize;
    let mut failed_total = 0usize;
    for file in files {
        let filename = file
            .relative_path
            .file_name()
            .map(|v| v.to_string_lossy().to_string())
            .unwrap_or_else(|| file.relative_path.display().to_string());
        let file_type = file
            .relative_path
            .extension()
            .map(|v| v.to_string_lossy().to_string())
            .filter(|v| !v.is_empty());
        for keyword in crate::kad::keyword::words(&filename) {
            items_total += 1;
            let keyword_id = crate::kad::keyword::keyword_hash(&keyword);
            tracing::info!(
                path = %file.relative_path.display(),
                hash = %file.file_hash_md4_hex,
                keyword = %keyword,
                "queueing shared file keyword publish"
            );
            if kad_cmd_tx
                .send(KadServiceCommand::PublishKeyword {
                    keyword: keyword_id,
                    keyword_label: Some(keyword.clone()),
                    file: file.file_id,
                    filename: filename.clone(),
                    file_size: file.file_size,
                    file_type: file_type.clone(),
                })
                .await
                .is_err()
            {
                failed_total += 1;
                publish_tracker.note_keyword_queue_failed(&file.file_hash_md4_hex);
                tracing::warn!(
                    path = %file.relative_path.display(),
                    hash = %file.file_hash_md4_hex,
                    keyword = %keyword,
                    "failed to queue shared file keyword publish"
                );
                continue;
            }
            queued_total += 1;
            publish_tracker.note_keyword_queued(&file.file_hash_md4_hex);
        }
    }
    (items_total, queued_total, failed_total)
}

fn now_unix_secs() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|value| value.as_secs())
}

fn cooldown_until(cooldown_secs: u64) -> Option<u64> {
    now_unix_secs().map(|now| now + cooldown_secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publish::SharedPublishTracker;
    use crate::share::SharedLibrary;
    use std::sync::Arc;
    use tokio::sync::{RwLock, mpsc};

    #[tokio::test]
    async fn start_reindex_rejects_duplicate_running_action() {
        let manager = SharedOpsManager::new(
            Arc::new(RwLock::new(SharedLibrary::default())),
            Arc::new(tokio::sync::Mutex::new(Config::default())),
            Arc::new(SharedPublishTracker::default()),
            mpsc::channel(1).0,
        );
        {
            let mut statuses = manager.statuses.lock().await;
            statuses.insert(
                SharedActionKind::Reindex,
                SharedActionStatus {
                    action: "reindex".to_string(),
                    state: "running".to_string(),
                    started_unix_secs: Some(1),
                    finished_unix_secs: None,
                    cooldown_until_unix_secs: None,
                    items_total: 0,
                    queued_total: 0,
                    failed_total: 0,
                    library_files_total: None,
                    reused_entries: None,
                    hashed_entries: None,
                    last_error: None,
                },
            );
        }

        let response = manager.start_reindex().await;
        assert!(!response.started);
        assert_eq!(
            response.reason,
            Some(SharedActionRejectReason::AlreadyRunning)
        );
        assert_eq!(response.status.state, "running");
    }

    #[tokio::test]
    async fn start_republish_sources_rejects_during_cooldown() {
        let manager = SharedOpsManager::new(
            Arc::new(RwLock::new(SharedLibrary::default())),
            Arc::new(tokio::sync::Mutex::new(Config::default())),
            Arc::new(SharedPublishTracker::default()),
            mpsc::channel(1).0,
        );
        {
            let mut statuses = manager.statuses.lock().await;
            statuses.insert(
                SharedActionKind::RepublishSources,
                SharedActionStatus {
                    action: "republish_sources".to_string(),
                    state: "succeeded".to_string(),
                    started_unix_secs: Some(1),
                    finished_unix_secs: Some(2),
                    cooldown_until_unix_secs: now_unix_secs().map(|value| value + 60),
                    items_total: 1,
                    queued_total: 1,
                    failed_total: 0,
                    library_files_total: Some(1),
                    reused_entries: None,
                    hashed_entries: None,
                    last_error: None,
                },
            );
        }

        let response = manager.start_republish_sources().await;
        assert!(!response.started);
        assert_eq!(
            response.reason,
            Some(SharedActionRejectReason::CooldownActive)
        );
        assert_eq!(response.status.state, "succeeded");
    }

    #[tokio::test]
    async fn republish_sources_fails_when_kad_service_disabled() {
        let mut config = Config::default();
        config.kad.service_enabled = false;
        let manager = SharedOpsManager::new(
            Arc::new(RwLock::new(SharedLibrary::default())),
            Arc::new(tokio::sync::Mutex::new(config)),
            Arc::new(SharedPublishTracker::default()),
            mpsc::channel(1).0,
        );

        let response = manager.start_republish_sources().await;
        assert!(response.started);

        tokio::task::yield_now().await;
        let snapshot = manager.action_snapshot().await;
        let status = snapshot
            .actions
            .into_iter()
            .find(|action| action.action == "republish_sources")
            .expect("republish_sources status");
        assert_eq!(status.state, "failed");
        assert_eq!(
            status.last_error.as_deref(),
            Some("KAD service is disabled")
        );
    }

    #[tokio::test]
    async fn republish_keywords_fails_when_kad_service_disabled() {
        let mut config = Config::default();
        config.kad.service_enabled = false;
        let manager = SharedOpsManager::new(
            Arc::new(RwLock::new(SharedLibrary::default())),
            Arc::new(tokio::sync::Mutex::new(config)),
            Arc::new(SharedPublishTracker::default()),
            mpsc::channel(1).0,
        );

        let response = manager.start_republish_keywords().await;
        assert!(response.started);

        tokio::task::yield_now().await;
        let snapshot = manager.action_snapshot().await;
        let status = snapshot
            .actions
            .into_iter()
            .find(|action| action.action == "republish_keywords")
            .expect("republish_keywords status");
        assert_eq!(status.state, "failed");
        assert_eq!(
            status.last_error.as_deref(),
            Some("KAD service is disabled")
        );
    }
}
