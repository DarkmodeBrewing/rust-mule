use crate::kad::{KadId, md4};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

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
    let runtime_dir = make_absolute(data_dir)?;
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
    for (root_index, root) in roots.iter().enumerate() {
        walk_root(root_index, root, &root.canonical_path, &mut out)?;
    }
    Ok(out)
}

pub fn index_shared_files(roots: &[SharedRoot]) -> Result<SharedLibrary> {
    let mut library = SharedLibrary::default();
    for file in enumerate_shared_files(roots)? {
        let file_id = hash_file_md4(&file.canonical_path)?;
        let file_hash_md4_hex = file_id.to_hex_lower();
        if let Some(existing_idx) = library.by_hash.get(&file_hash_md4_hex).copied() {
            let existing = &library.files[existing_idx];
            tracing::warn!(
                file = %file.canonical_path.display(),
                existing = %existing.canonical_path.display(),
                hash = %file_hash_md4_hex,
                "duplicate shared file hash; keeping first path for uploader"
            );
            continue;
        }
        let idx = library.files.len();
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
    Ok(library)
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
    out: &mut Vec<SharedFile>,
) -> Result<()> {
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
            walk_root(root_index, root, &path, out)?;
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
        canonicalize_share_roots, enumerate_shared_files, index_shared_files, read_shared_block,
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
}
