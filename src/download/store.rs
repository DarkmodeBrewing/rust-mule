use crate::download::errors::DownloadStoreError;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, DownloadStoreError>;

// iMule/eMule part.met versions are 0xE0/0xE2. We keep the same default marker
// in our metadata model while using a Rust-native serialized structure.
pub const PART_MET_VERSION: u8 = 0xE0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartState {
    Queued,
    Downloading,
    Paused,
    Completing,
    Completed,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartMet {
    pub version: u8,
    pub part_number: u16,
    pub file_name: String,
    pub file_size: u64,
    pub file_hash_md4_hex: String,
    pub state: PartState,
    pub downloaded_bytes: u64,
    #[serde(default)]
    pub missing_ranges: Vec<ByteRange>,
    #[serde(default)]
    pub inflight_ranges: Vec<ByteRange>,
    #[serde(default)]
    pub retry_count: u32,
    #[serde(default)]
    pub last_error: Option<String>,
    pub created_unix_secs: u64,
    pub updated_unix_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownMetEntry {
    pub file_name: String,
    pub file_size: u64,
    pub file_hash_md4_hex: String,
    pub completed_unix_secs: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadedMetSource {
    Primary,
    Backup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredDownload {
    pub met_path: PathBuf,
    pub part_path: PathBuf,
    pub met: PartMet,
    pub source: LoadedMetSource,
}

pub fn met_path_for_part(download_dir: &Path, part_number: u16) -> PathBuf {
    download_dir.join(format!("{part_number:03}.part.met"))
}

pub fn part_path_for_part(download_dir: &Path, part_number: u16) -> PathBuf {
    download_dir.join(format!("{part_number:03}.part"))
}

pub async fn allocate_next_part_number(download_dir: &Path) -> Result<u16> {
    let mut used = std::collections::BTreeSet::<u16>::new();
    let mut rd =
        tokio::fs::read_dir(download_dir)
            .await
            .map_err(|source| DownloadStoreError::ReadDir {
                path: download_dir.to_path_buf(),
                source,
            })?;
    while let Some(entry) = rd
        .next_entry()
        .await
        .map_err(|source| DownloadStoreError::ReadDir {
            path: download_dir.to_path_buf(),
            source,
        })?
    {
        if let Some(num) = parse_part_number(&entry.path()) {
            used.insert(num);
        }
    }

    for n in 1..=u16::MAX {
        if !used.contains(&n) {
            return Ok(n);
        }
    }
    Err(DownloadStoreError::WriteFile {
        path: download_dir.to_path_buf(),
        source: std::io::Error::other("no free part number available"),
    })
}

pub async fn save_part_met(path: &Path, met: &PartMet) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(met)
        .map_err(|source| DownloadStoreError::Serialize { source })?;

    if path.exists() {
        let bak = backup_path(path);
        tokio::fs::copy(path, &bak)
            .await
            .map_err(|source| DownloadStoreError::Copy {
                from: path.to_path_buf(),
                to: bak,
                source,
            })?;
    }

    let tmp = tmp_path(path);
    tokio::fs::write(&tmp, &bytes)
        .await
        .map_err(|source| DownloadStoreError::WriteFile {
            path: tmp.clone(),
            source,
        })?;
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|source| DownloadStoreError::Rename {
            from: tmp,
            to: path.to_path_buf(),
            source,
        })?;
    Ok(())
}

pub async fn load_part_met_with_fallback(path: &Path) -> Result<(PartMet, LoadedMetSource)> {
    match load_part_met(path).await {
        Ok(met) => Ok((met, LoadedMetSource::Primary)),
        Err(primary_err) => {
            let bak = backup_path(path);
            if bak.exists() {
                match load_part_met(&bak).await {
                    Ok(met) => Ok((met, LoadedMetSource::Backup)),
                    Err(_) => Err(primary_err),
                }
            } else {
                Err(primary_err)
            }
        }
    }
}

pub async fn scan_recoverable_downloads(download_dir: &Path) -> Result<Vec<RecoveredDownload>> {
    let mut out = Vec::new();
    let mut rd =
        tokio::fs::read_dir(download_dir)
            .await
            .map_err(|source| DownloadStoreError::ReadDir {
                path: download_dir.to_path_buf(),
                source,
            })?;

    while let Some(entry) = rd
        .next_entry()
        .await
        .map_err(|source| DownloadStoreError::ReadDir {
            path: download_dir.to_path_buf(),
            source,
        })?
    {
        let path = entry.path();
        if !is_primary_part_met_file(&path) {
            continue;
        }

        match load_part_met_with_fallback(&path).await {
            Ok((met, source)) => {
                out.push(RecoveredDownload {
                    part_path: part_path_from_met_path(&path),
                    met_path: path,
                    met,
                    source,
                });
            }
            Err(err) => {
                tracing::warn!(path = %path.display(), error = %err, "failed to recover part metadata");
            }
        }
    }
    out.sort_by_key(|r| r.met.part_number);
    Ok(out)
}

pub async fn load_known_met_entries(path: &Path) -> Result<Vec<KnownMetEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|source| DownloadStoreError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
    serde_json::from_slice(&bytes).map_err(|source| DownloadStoreError::ParseKnown {
        path: path.to_path_buf(),
        source,
    })
}

pub async fn append_known_met_entry(path: &Path, entry: KnownMetEntry) -> Result<bool> {
    let mut entries = load_known_met_entries(path).await?;
    if entries
        .iter()
        .any(|e| e.file_hash_md4_hex == entry.file_hash_md4_hex && e.file_size == entry.file_size)
    {
        return Ok(false);
    }
    entries.push(entry);
    save_known_met_entries(path, &entries).await?;
    Ok(true)
}

async fn save_known_met_entries(path: &Path, entries: &[KnownMetEntry]) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(entries)
        .map_err(|source| DownloadStoreError::Serialize { source })?;
    let tmp = tmp_path(path);
    tokio::fs::write(&tmp, &bytes)
        .await
        .map_err(|source| DownloadStoreError::WriteFile {
            path: tmp.clone(),
            source,
        })?;
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|source| DownloadStoreError::Rename {
            from: tmp,
            to: path.to_path_buf(),
            source,
        })?;
    Ok(())
}

fn is_primary_part_met_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    name.ends_with(".part.met")
}

fn parse_part_number(path: &Path) -> Option<u16> {
    let name = path.file_name()?.to_str()?;
    let stem = name
        .strip_suffix(".part.met")
        .or_else(|| name.strip_suffix(".part"))?;
    if !stem.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    stem.parse::<u16>().ok()
}

fn part_path_from_met_path(path: &Path) -> PathBuf {
    let mut p = path.to_path_buf();
    if let Some(name) = path.file_name().and_then(|n| n.to_str())
        && let Some(stem) = name.strip_suffix(".part.met")
    {
        p.set_file_name(format!("{stem}.part"));
        return p;
    }
    p
}

fn backup_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".bak");
    PathBuf::from(s)
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".tmp");
    PathBuf::from(s)
}

async fn load_part_met(path: &Path) -> Result<PartMet> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|source| DownloadStoreError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
    serde_json::from_slice(&bytes).map_err(|source| DownloadStoreError::ParseMet {
        path: path.to_path_buf(),
        source,
    })
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
        p.push(format!("rust-mule-download-store-{tag}-{nanos}"));
        p
    }

    fn sample_met(part_number: u16) -> PartMet {
        PartMet {
            version: PART_MET_VERSION,
            part_number,
            file_name: format!("file-{part_number}.bin"),
            file_size: 12345,
            file_hash_md4_hex: "0123456789abcdef0123456789abcdef".to_string(),
            state: PartState::Queued,
            downloaded_bytes: 100,
            missing_ranges: vec![ByteRange {
                start: 100,
                end: 12344,
            }],
            inflight_ranges: Vec::new(),
            retry_count: 0,
            last_error: None,
            created_unix_secs: 1,
            updated_unix_secs: 2,
        }
    }

    #[tokio::test]
    async fn save_and_load_part_met_roundtrip() {
        let root = temp_dir("roundtrip");
        tokio::fs::create_dir_all(&root).await.expect("mkdir");
        let path = root.join("001.part.met");

        let met = sample_met(1);
        save_part_met(&path, &met).await.expect("save");
        let (loaded, src) = load_part_met_with_fallback(&path).await.expect("load");
        assert_eq!(src, LoadedMetSource::Primary);
        assert_eq!(loaded, met);

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn load_falls_back_to_backup_when_primary_is_corrupt() {
        let root = temp_dir("fallback");
        tokio::fs::create_dir_all(&root).await.expect("mkdir");
        let path = root.join("002.part.met");
        let bak = backup_path(&path);

        let met = sample_met(2);
        save_part_met(&bak, &met).await.expect("save bak");
        tokio::fs::write(&path, b"{not-json")
            .await
            .expect("write corrupt");

        let (loaded, src) = load_part_met_with_fallback(&path).await.expect("load");
        assert_eq!(src, LoadedMetSource::Backup);
        assert_eq!(loaded.part_number, 2);

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn scan_recovers_primary_part_met_files_only() {
        let root = temp_dir("scan");
        tokio::fs::create_dir_all(&root).await.expect("mkdir");
        let met1 = sample_met(1);
        let met2 = sample_met(2);

        save_part_met(&root.join("001.part.met"), &met1)
            .await
            .expect("save 1");
        save_part_met(&root.join("002.part.met"), &met2)
            .await
            .expect("save 2");
        save_part_met(&root.join("skip.part.met.bak"), &sample_met(3))
            .await
            .expect("save bak");

        let recovered = scan_recoverable_downloads(&root).await.expect("scan");
        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered[0].met.part_number, 1);
        assert_eq!(recovered[1].met.part_number, 2);
        assert_eq!(
            recovered[0]
                .part_path
                .file_name()
                .and_then(|n| n.to_str())
                .expect("name"),
            "001.part"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn allocate_next_part_number_uses_lowest_free_slot() {
        let root = temp_dir("alloc");
        tokio::fs::create_dir_all(&root).await.expect("mkdir");
        tokio::fs::write(root.join("001.part.met"), b"{}")
            .await
            .expect("write 1");
        tokio::fs::write(root.join("003.part"), b"")
            .await
            .expect("write 3");

        let n = allocate_next_part_number(&root).await.expect("alloc");
        assert_eq!(n, 2);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn allocate_next_part_number_handles_four_digit_parts() {
        let root = temp_dir("alloc-4digit");
        tokio::fs::create_dir_all(&root).await.expect("mkdir");
        for part in 1..=1000u16 {
            tokio::fs::write(root.join(format!("{part}.part")), b"")
                .await
                .expect("write part");
        }

        let n = allocate_next_part_number(&root).await.expect("alloc");
        assert_eq!(n, 1001);
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn append_known_met_entry_persists_and_deduplicates_by_hash_and_size() {
        let root = temp_dir("known-met");
        tokio::fs::create_dir_all(&root).await.expect("mkdir");
        let path = root.join("known.met");
        let entry = KnownMetEntry {
            file_name: "a.bin".to_string(),
            file_size: 10,
            file_hash_md4_hex: "0123456789abcdef0123456789abcdef".to_string(),
            completed_unix_secs: 1,
        };

        let inserted = append_known_met_entry(&path, entry.clone())
            .await
            .expect("append");
        assert!(inserted);
        let inserted_again = append_known_met_entry(&path, entry)
            .await
            .expect("append again");
        assert!(!inserted_again);

        let loaded = load_known_met_entries(&path).await.expect("load");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].file_name, "a.bin");
        let _ = std::fs::remove_dir_all(root);
    }
}
