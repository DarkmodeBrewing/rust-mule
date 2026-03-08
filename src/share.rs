use crate::kad::{KadId, md4};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedRoot {
    pub configured_path: String,
    pub canonical_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedFile {
    pub root_index: usize,
    pub canonical_path: PathBuf,
    pub relative_path: PathBuf,
    pub file_size: u64,
    pub modified_unix_secs: u64,
    pub modified_subsec_nanos: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedLibraryFile {
    pub root_index: usize,
    pub canonical_path: PathBuf,
    pub relative_path: PathBuf,
    pub file_size: u64,
    pub file_id: KadId,
    pub file_hash_md4_hex: String,
}

#[derive(Debug, Clone, Default)]
pub struct SharedLibrary {
    files: Vec<SharedLibraryFile>,
    by_hash: HashMap<String, usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedLibraryBuildStats {
    pub reused_entries: usize,
    pub hashed_entries: usize,
}

#[derive(Debug, Clone)]
pub struct SharedLibraryBuild {
    pub library: SharedLibrary,
    pub stats: SharedLibraryBuildStats,
}

impl SharedLibrary {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn files(&self) -> &[SharedLibraryFile] {
        &self.files
    }

    pub fn get_by_hash_hex(&self, hash_hex: &str) -> Option<&SharedLibraryFile> {
        self.by_hash
            .get(&hash_hex.to_ascii_lowercase())
            .and_then(|idx| self.files.get(*idx))
    }
}

#[derive(Debug)]
pub enum ShareError {
    EmptyPath,
    CurrentDir(std::io::Error),
    Canonicalize {
        path: PathBuf,
        source: std::io::Error,
    },
    Metadata {
        path: PathBuf,
        source: std::io::Error,
    },
    OpenFile {
        path: PathBuf,
        source: std::io::Error,
    },
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    ReadDir {
        path: PathBuf,
        source: std::io::Error,
    },
    UnsafeRoot(PathBuf),
    OverlappingRoots {
        first: PathBuf,
        second: PathBuf,
    },
    OutsideRoot {
        path: PathBuf,
        root: PathBuf,
    },
}

impl std::fmt::Display for ShareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPath => write!(f, "share root path must not be empty"),
            Self::CurrentDir(_) => write!(f, "failed to resolve current working directory"),
            Self::Canonicalize { path, .. } => {
                write!(f, "failed to resolve share root '{}'", path.display())
            }
            Self::Metadata { path, .. } => {
                write!(f, "failed to stat shared path '{}'", path.display())
            }
            Self::OpenFile { path, .. } => {
                write!(f, "failed to open shared file '{}'", path.display())
            }
            Self::ReadFile { path, .. } => {
                write!(f, "failed to read shared file '{}'", path.display())
            }
            Self::ReadDir { path, .. } => {
                write!(f, "failed to enumerate shared path '{}'", path.display())
            }
            Self::UnsafeRoot(path) => write!(f, "unsafe share root '{}'", path.display()),
            Self::OverlappingRoots { first, second } => write!(
                f,
                "overlapping share roots '{}' and '{}'",
                first.display(),
                second.display()
            ),
            Self::OutsideRoot { path, root } => write!(
                f,
                "shared file '{}' escaped share root '{}'",
                path.display(),
                root.display()
            ),
        }
    }
}

impl std::error::Error for ShareError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentDir(source) => Some(source),
            Self::Canonicalize { source, .. } => Some(source),
            Self::Metadata { source, .. } => Some(source),
            Self::OpenFile { source, .. } => Some(source),
            Self::ReadFile { source, .. } => Some(source),
            Self::ReadDir { source, .. } => Some(source),
            Self::EmptyPath
            | Self::UnsafeRoot(_)
            | Self::OverlappingRoots { .. }
            | Self::OutsideRoot { .. } => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, ShareError>;

pub fn canonicalize_share_roots(
    share_roots: &[String],
    data_dir: &Path,
) -> Result<Vec<SharedRoot>> {
    let runtime_dir = canonicalize_runtime_dir(data_dir)?;
    let mut out: Vec<SharedRoot> = Vec::with_capacity(share_roots.len());

    for raw in share_roots {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ShareError::EmptyPath);
        }
        let canonical = canonicalize_existing_dir(Path::new(trimmed))?;
        ensure_safe_root(&canonical, &runtime_dir)?;

        for existing in &out {
            if paths_overlap(&canonical, &existing.canonical_path) {
                return Err(ShareError::OverlappingRoots {
                    first: existing.canonical_path.clone(),
                    second: canonical,
                });
            }
        }

        out.push(SharedRoot {
            configured_path: trimmed.to_string(),
            canonical_path: canonical,
        });
    }

    Ok(out)
}

pub fn enumerate_shared_files(roots: &[SharedRoot]) -> Result<Vec<SharedFile>> {
    let mut out = Vec::new();
    let mut visited_dirs = HashSet::new();
    for (root_index, root) in roots.iter().enumerate() {
        walk_root(
            root_index,
            root,
            &root.canonical_path,
            &mut visited_dirs,
            &mut out,
        )?;
    }
    Ok(out)
}

pub fn index_shared_files(roots: &[SharedRoot]) -> Result<SharedLibrary> {
    let mut library = SharedLibrary::default();
    for file in enumerate_shared_files(roots)? {
        let file_hash_md4_hex = hash_file_md4(&file.canonical_path)?.to_hex_lower();
        insert_library_file(&mut library, file, file_hash_md4_hex);
    }
    Ok(library)
}

pub async fn load_or_rebuild_shared_library(
    roots: &[SharedRoot],
    cache_path: &Path,
) -> Result<SharedLibraryBuild> {
    let cache = load_index_cache(cache_path).await;
    let mut library = SharedLibrary::default();
    let mut cache_entries = HashMap::new();
    let mut reused_entries = 0usize;
    let mut hashed_entries = 0usize;

    for file in enumerate_shared_files(roots)? {
        let cache_entry = if let Some(entry) = cache
            .as_ref()
            .and_then(|cache| cache.entries.get(&file.canonical_path))
            .filter(|entry| {
                entry.file_size == file.file_size
                    && entry.modified_unix_secs == file.modified_unix_secs
                    && entry.modified_subsec_nanos == file.modified_subsec_nanos
                    && KadId::from_hex(&entry.file_hash_md4_hex).is_ok()
            }) {
            reused_entries += 1;
            tracing::info!(
                path = %file.relative_path.display(),
                hash = %entry.file_hash_md4_hex,
                size = file.file_size,
                "shared library cache reused file hash"
            );
            SharedLibraryCacheEntry {
                file_size: file.file_size,
                modified_unix_secs: file.modified_unix_secs,
                modified_subsec_nanos: file.modified_subsec_nanos,
                file_hash_md4_hex: entry.file_hash_md4_hex.clone(),
            }
        } else {
            if let Some(entry) = cache
                .as_ref()
                .and_then(|cache| cache.entries.get(&file.canonical_path))
                .filter(|entry| {
                    entry.file_size == file.file_size
                        && entry.modified_unix_secs == file.modified_unix_secs
                        && entry.modified_subsec_nanos == file.modified_subsec_nanos
                        && KadId::from_hex(&entry.file_hash_md4_hex).is_err()
                })
            {
                tracing::warn!(
                    path = %file.relative_path.display(),
                    hash = %entry.file_hash_md4_hex,
                    "shared library cache entry had invalid hash; rehashing file"
                );
            }
            hashed_entries += 1;
            let hash = hash_file_md4(&file.canonical_path)?.to_hex_lower();
            tracing::info!(
                path = %file.relative_path.display(),
                hash = %hash,
                size = file.file_size,
                "shared library hashed file"
            );
            SharedLibraryCacheEntry {
                file_size: file.file_size,
                modified_unix_secs: file.modified_unix_secs,
                modified_subsec_nanos: file.modified_subsec_nanos,
                file_hash_md4_hex: hash,
            }
        };
        cache_entries.insert(file.canonical_path.clone(), cache_entry.clone());
        insert_library_file(&mut library, file, cache_entry.file_hash_md4_hex);
    }

    store_index_cache(cache_path, &cache_entries).await;

    Ok(SharedLibraryBuild {
        library,
        stats: SharedLibraryBuildStats {
            reused_entries,
            hashed_entries,
        },
    })
}

pub fn read_shared_block(file: &SharedLibraryFile, start: u64, end: u64) -> Result<Vec<u8>> {
    if end < start || end >= file.file_size {
        return Err(ShareError::OutsideRoot {
            path: file.canonical_path.clone(),
            root: file.canonical_path.clone(),
        });
    }

    let mut handle =
        std::fs::File::open(&file.canonical_path).map_err(|source| ShareError::OpenFile {
            path: file.canonical_path.clone(),
            source,
        })?;
    handle
        .seek(SeekFrom::Start(start))
        .map_err(|source| ShareError::ReadFile {
            path: file.canonical_path.clone(),
            source,
        })?;
    let len = end.saturating_sub(start).saturating_add(1) as usize;
    let mut buf = vec![0u8; len];
    handle
        .read_exact(&mut buf)
        .map_err(|source| ShareError::ReadFile {
            path: file.canonical_path.clone(),
            source,
        })?;
    Ok(buf)
}

fn walk_root(
    root_index: usize,
    root: &SharedRoot,
    dir: &Path,
    visited_dirs: &mut HashSet<PathBuf>,
    out: &mut Vec<SharedFile>,
) -> Result<()> {
    let canonical_dir = std::fs::canonicalize(dir).map_err(|source| ShareError::Canonicalize {
        path: dir.to_path_buf(),
        source,
    })?;
    if !visited_dirs.insert(canonical_dir.clone()) {
        return Ok(());
    }
    let rd = std::fs::read_dir(dir).map_err(|source| ShareError::ReadDir {
        path: dir.to_path_buf(),
        source,
    })?;

    for entry in rd {
        let entry = entry.map_err(|source| ShareError::ReadDir {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let canonical_path =
            std::fs::canonicalize(&path).map_err(|source| ShareError::Canonicalize {
                path: path.clone(),
                source,
            })?;
        if !canonical_path.starts_with(&root.canonical_path) {
            return Err(ShareError::OutsideRoot {
                path: canonical_path,
                root: root.canonical_path.clone(),
            });
        }

        let metadata = entry.metadata().map_err(|source| ShareError::Metadata {
            path: path.clone(),
            source,
        })?;
        if metadata.is_dir() {
            walk_root(root_index, root, &canonical_path, visited_dirs, out)?;
            continue;
        }
        if !metadata.is_file() {
            continue;
        }

        let relative_path = canonical_path
            .strip_prefix(&root.canonical_path)
            .map_err(|_| ShareError::OutsideRoot {
                path: canonical_path.clone(),
                root: root.canonical_path.clone(),
            })?
            .to_path_buf();
        out.push(SharedFile {
            root_index,
            canonical_path,
            relative_path,
            file_size: metadata.len(),
            modified_unix_secs: modified_unix_secs(&metadata),
            modified_subsec_nanos: modified_subsec_nanos(&metadata),
        });
    }
    Ok(())
}

fn canonicalize_existing_dir(path: &Path) -> Result<PathBuf> {
    let absolute = make_absolute(path)?;
    let canonical =
        std::fs::canonicalize(&absolute).map_err(|source| ShareError::Canonicalize {
            path: absolute.clone(),
            source,
        })?;
    let metadata = std::fs::metadata(&canonical).map_err(|source| ShareError::Metadata {
        path: canonical.clone(),
        source,
    })?;
    if !metadata.is_dir() {
        return Err(ShareError::UnsafeRoot(canonical));
    }
    Ok(canonical)
}

fn canonicalize_runtime_dir(path: &Path) -> Result<PathBuf> {
    let absolute = make_absolute(path)?;
    match std::fs::canonicalize(&absolute) {
        Ok(path) => Ok(path),
        Err(_) => Ok(absolute),
    }
}

fn hash_file_md4(path: &Path) -> Result<KadId> {
    let mut file = std::fs::File::open(path).map_err(|source| ShareError::OpenFile {
        path: path.to_path_buf(),
        source,
    })?;
    let digest = md4::digest_reader(&mut file).map_err(|source| ShareError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(KadId(digest))
}

fn insert_library_file(library: &mut SharedLibrary, file: SharedFile, file_hash_md4_hex: String) {
    if let Some(existing_idx) = library.by_hash.get(&file_hash_md4_hex).copied() {
        let existing = &library.files[existing_idx];
        tracing::warn!(
            file = %file.canonical_path.display(),
            existing = %existing.canonical_path.display(),
            hash = %file_hash_md4_hex,
            "duplicate shared file hash; keeping first path for uploader"
        );
        return;
    }
    let idx = library.files.len();
    let file_id = match KadId::from_hex(&file_hash_md4_hex) {
        Ok(id) => id,
        Err(err) => {
            tracing::warn!(
                file = %file.canonical_path.display(),
                hash = %file_hash_md4_hex,
                error = %err,
                "invalid shared file hash; skipping library entry"
            );
            return;
        }
    };
    library.by_hash.insert(file_hash_md4_hex.clone(), idx);
    library.files.push(SharedLibraryFile {
        root_index: file.root_index,
        canonical_path: file.canonical_path,
        relative_path: file.relative_path,
        file_size: file.file_size,
        file_id,
        file_hash_md4_hex,
    });
}

fn modified_unix_secs(metadata: &std::fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |dur| dur.as_secs())
}

fn modified_subsec_nanos(metadata: &std::fs::Metadata) -> u32 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |dur| dur.subsec_nanos())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SharedLibraryCache {
    entries: HashMap<PathBuf, SharedLibraryCacheEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SharedLibraryCacheEntry {
    file_size: u64,
    modified_unix_secs: u64,
    modified_subsec_nanos: u32,
    file_hash_md4_hex: String,
}

async fn load_index_cache(path: &Path) -> Option<SharedLibraryCache> {
    let bytes = match tokio::fs::read(path).await {
        Ok(bytes) => bytes,
        Err(_) => return None,
    };
    match serde_json::from_slice::<SharedLibraryCache>(&bytes) {
        Ok(cache) => Some(cache),
        Err(err) => {
            tracing::warn!(path = %path.display(), error = %err, "shared library cache parse failed; rebuilding");
            None
        }
    }
}

async fn store_index_cache(path: &Path, entries: &HashMap<PathBuf, SharedLibraryCacheEntry>) {
    let cache = SharedLibraryCache {
        entries: entries.clone(),
    };
    let bytes = match serde_json::to_vec_pretty(&cache) {
        Ok(bytes) => bytes,
        Err(err) => {
            tracing::warn!(path = %path.display(), error = %err, "shared library cache serialize failed");
            return;
        }
    };
    let tmp = path.with_extension("tmp");
    if let Err(err) = tokio::fs::write(&tmp, &bytes).await {
        tracing::warn!(path = %tmp.display(), error = %err, "shared library cache write failed");
        return;
    }
    if let Err(err) = tokio::fs::rename(&tmp, path).await {
        tracing::warn!(
            from = %tmp.display(),
            to = %path.display(),
            error = %err,
            "shared library cache rename failed"
        );
    }
}

fn make_absolute(path: &Path) -> Result<PathBuf> {
    let cwd = std::env::current_dir().map_err(ShareError::CurrentDir)?;
    Ok(if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    })
}

fn ensure_safe_root(candidate: &Path, runtime_dir: &Path) -> Result<()> {
    if is_root_path(candidate) || is_known_unsafe_system_root(candidate) {
        return Err(ShareError::UnsafeRoot(candidate.to_path_buf()));
    }
    if paths_overlap(candidate, runtime_dir) {
        return Err(ShareError::UnsafeRoot(candidate.to_path_buf()));
    }
    Ok(())
}

fn paths_overlap(a: &Path, b: &Path) -> bool {
    a == b || a.starts_with(b) || b.starts_with(a)
}

fn is_root_path(path: &Path) -> bool {
    path.parent().is_none()
}

#[cfg(unix)]
fn is_known_unsafe_system_root(path: &Path) -> bool {
    matches!(
        path.to_str(),
        Some(
            "/bin"
                | "/boot"
                | "/dev"
                | "/etc"
                | "/home"
                | "/lib"
                | "/lib64"
                | "/proc"
                | "/root"
                | "/run"
                | "/sbin"
                | "/srv"
                | "/sys"
                | "/tmp"
                | "/usr"
                | "/var"
        )
    )
}

#[cfg(not(unix))]
fn is_known_unsafe_system_root(_path: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::{
        canonicalize_share_roots, enumerate_shared_files, index_shared_files,
        load_or_rebuild_shared_library, read_shared_block,
    };

    fn mktemp(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "rust_mule_share_test_{}_{}_{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        root
    }

    #[test]
    fn rejects_empty_share_root() {
        let data_dir = mktemp("empty_data");
        let err = canonicalize_share_roots(&["  ".to_string()], &data_dir).expect_err("empty");
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn rejects_runtime_data_dir_overlap() {
        let root = mktemp("runtime_overlap");
        let data_dir = root.join("data");
        let shared = root.join("shared");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        std::fs::create_dir_all(&shared).expect("shared dir");

        let err = canonicalize_share_roots(&[data_dir.display().to_string()], &data_dir)
            .expect_err("unsafe");
        assert!(err.to_string().contains("unsafe share root"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_runtime_data_dir_overlap_when_data_dir_is_symlinked() {
        let root = mktemp("runtime_overlap_symlink");
        let data_real = root.join("data-real");
        let data_link = root.join("data-link");
        std::fs::create_dir_all(&data_real).expect("data dir");
        std::os::unix::fs::symlink(&data_real, &data_link).expect("symlink data dir");

        let err = canonicalize_share_roots(&[data_real.display().to_string()], &data_link)
            .expect_err("unsafe");
        assert!(err.to_string().contains("unsafe share root"));
    }

    #[test]
    fn rejects_overlapping_roots() {
        let root = mktemp("overlap");
        let data_dir = root.join("data");
        let shared = root.join("shared");
        let nested = shared.join("nested");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        std::fs::create_dir_all(&nested).expect("nested dir");

        let err = canonicalize_share_roots(
            &[shared.display().to_string(), nested.display().to_string()],
            &data_dir,
        )
        .expect_err("overlap");
        assert!(err.to_string().contains("overlapping share roots"));
    }

    #[test]
    fn enumerates_files_under_valid_root() {
        let root = mktemp("enumerate");
        let data_dir = root.join("data");
        let shared = root.join("shared");
        let nested = shared.join("nested");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        std::fs::create_dir_all(&nested).expect("nested dir");
        std::fs::write(shared.join("a.bin"), b"abc").expect("write a");
        std::fs::write(nested.join("b.bin"), b"hello").expect("write b");

        let roots = canonicalize_share_roots(&[shared.display().to_string()], &data_dir)
            .expect("valid root");
        let mut files = enumerate_shared_files(&roots).expect("enumerate");
        files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].relative_path, std::path::PathBuf::from("a.bin"));
        assert_eq!(files[0].file_size, 3);
        assert_eq!(
            files[1].relative_path,
            std::path::PathBuf::from("nested").join("b.bin")
        );
        assert_eq!(files[1].file_size, 5);
    }

    #[cfg(unix)]
    #[test]
    fn enumerate_shared_files_skips_cyclic_directory_symlink() {
        let root = mktemp("loop_symlink");
        let data_dir = root.join("data");
        let shared = root.join("shared");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        std::fs::create_dir_all(&shared).expect("shared dir");
        std::fs::write(shared.join("real.bin"), b"abc").expect("write file");
        std::os::unix::fs::symlink(&shared, shared.join("loop")).expect("symlink loop");

        let roots = canonicalize_share_roots(&[shared.display().to_string()], &data_dir)
            .expect("valid root");
        let files = enumerate_shared_files(&roots).expect("enumerate");

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, std::path::PathBuf::from("real.bin"));
    }

    #[test]
    fn indexes_shared_files_with_md4() {
        let root = mktemp("index");
        let data_dir = root.join("data");
        let shared = root.join("shared");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        std::fs::create_dir_all(&shared).expect("shared dir");
        std::fs::write(shared.join("hello.txt"), b"hello world").expect("write file");

        let roots = canonicalize_share_roots(&[shared.display().to_string()], &data_dir)
            .expect("valid root");
        let library = index_shared_files(&roots).expect("index");

        assert_eq!(library.len(), 1);
        assert_eq!(
            library.files()[0].file_hash_md4_hex,
            "aa010fbc1d14c795d86ef98c95479d17"
        );
    }

    #[test]
    fn reads_requested_shared_block() {
        let root = mktemp("block");
        let data_dir = root.join("data");
        let shared = root.join("shared");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        std::fs::create_dir_all(&shared).expect("shared dir");
        std::fs::write(shared.join("chunk.bin"), b"0123456789abcdef").expect("write file");

        let roots = canonicalize_share_roots(&[shared.display().to_string()], &data_dir)
            .expect("valid root");
        let library = index_shared_files(&roots).expect("index");
        let block = read_shared_block(&library.files()[0], 4, 9).expect("read block");

        assert_eq!(block, b"456789");
    }

    #[tokio::test]
    async fn reuses_cached_hash_for_unchanged_file() {
        let root = mktemp("cache_reuse");
        let data_dir = root.join("data");
        let shared = root.join("shared");
        let cache = data_dir.join("shared_library.json");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        std::fs::create_dir_all(&shared).expect("shared dir");
        std::fs::write(shared.join("same.bin"), b"same-content").expect("write file");

        let roots = canonicalize_share_roots(&[shared.display().to_string()], &data_dir)
            .expect("valid root");
        let first = load_or_rebuild_shared_library(&roots, &cache)
            .await
            .expect("first build");
        let second = load_or_rebuild_shared_library(&roots, &cache)
            .await
            .expect("second build");

        assert_eq!(first.stats.hashed_entries, 1);
        assert_eq!(second.stats.reused_entries, 1);
        assert_eq!(second.stats.hashed_entries, 0);
    }

    #[tokio::test]
    async fn rehashes_file_after_change() {
        let root = mktemp("cache_invalidate");
        let data_dir = root.join("data");
        let shared = root.join("shared");
        let cache = data_dir.join("shared_library.json");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        std::fs::create_dir_all(&shared).expect("shared dir");
        let file_path = shared.join("change.bin");
        std::fs::write(&file_path, b"before").expect("write file");

        let roots = canonicalize_share_roots(&[shared.display().to_string()], &data_dir)
            .expect("valid root");
        let first = load_or_rebuild_shared_library(&roots, &cache)
            .await
            .expect("first build");
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::fs::write(&file_path, b"after-change").expect("rewrite file");
        let second = load_or_rebuild_shared_library(&roots, &cache)
            .await
            .expect("second build");

        assert_eq!(first.stats.hashed_entries, 1);
        assert_eq!(second.stats.hashed_entries, 1);
        assert_eq!(second.stats.reused_entries, 0);
        assert_ne!(
            first.library.files()[0].file_hash_md4_hex,
            second.library.files()[0].file_hash_md4_hex
        );
    }

    #[tokio::test]
    async fn invalid_cached_hash_is_rehashed() {
        let root = mktemp("cache_invalid_hash");
        let data_dir = root.join("data");
        let shared = root.join("shared");
        let cache = data_dir.join("shared_library.json");
        std::fs::create_dir_all(&data_dir).expect("data dir");
        std::fs::create_dir_all(&shared).expect("shared dir");
        let file_path = shared.join("same.bin");
        std::fs::write(&file_path, b"same-content").expect("write file");

        let roots = canonicalize_share_roots(&[shared.display().to_string()], &data_dir)
            .expect("valid root");
        let first = load_or_rebuild_shared_library(&roots, &cache)
            .await
            .expect("first build");

        let mut cache_json: serde_json::Value =
            serde_json::from_slice(&tokio::fs::read(&cache).await.expect("cache read"))
                .expect("cache json");
        let canonical = std::fs::canonicalize(&file_path).expect("canonical file");
        let entry = cache_json["entries"]
            .get_mut(canonical.to_str().expect("utf8 path"))
            .expect("cache entry");
        entry["file_hash_md4_hex"] = serde_json::Value::String("not-a-valid-md4".to_string());
        tokio::fs::write(
            &cache,
            serde_json::to_vec_pretty(&cache_json).expect("cache bytes"),
        )
        .await
        .expect("cache write");

        let second = load_or_rebuild_shared_library(&roots, &cache)
            .await
            .expect("second build");

        assert_eq!(
            first.library.files()[0].file_hash_md4_hex,
            second.library.files()[0].file_hash_md4_hex
        );
        assert_eq!(second.stats.hashed_entries, 1);
        assert_eq!(second.stats.reused_entries, 0);
    }
}
