// src/engine/io/source.rs
//
// Image source types and safe file loading.
//
// Owns the `Source` enum, the two guard types (`MmapGuard`, `MemoryGuard`),
// and the `load_file_safe` entry point that picks between memory-resident
// and memory-mapped strategies based on file size.

use super::super::memory;
use crate::error::LazyImageError;
use memmap2::Mmap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

/// Files at or below this size are read into memory instead of mmap'd.
/// This eliminates SIGBUS/SIGSEGV risk for the vast majority of images.
/// Only files above this threshold use mmap with advisory lock protection.
const MMAP_SIZE_THRESHOLD: u64 = 256 * 1024 * 1024; // 256 MB

/// Guard that holds a memory-mapped file and its file descriptor.
/// Keeping the fd open retains advisory locks (flock) on Unix systems,
/// preventing cooperative writers from modifying the file while it's mapped.
pub struct MmapGuard {
    mmap: Mmap,
    _file: File, // retains fd → retains flock on Unix
}

impl std::fmt::Debug for MmapGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MmapGuard")
            .field("len", &self.mmap.len())
            .finish()
    }
}

impl std::ops::Deref for MmapGuard {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.mmap
    }
}

impl AsRef<[u8]> for MmapGuard {
    fn as_ref(&self) -> &[u8] {
        &self.mmap
    }
}

/// Guard for file-backed in-memory data.
/// `reserved_bytes` tracks heap bytes that must be budgeted when this source
/// later enters the decode/encode pipeline.
pub struct MemoryGuard {
    data: Vec<u8>,
    reserved_bytes: u64,
}

impl MemoryGuard {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            reserved_bytes: 0,
        }
    }

    fn with_reserved_bytes(data: Vec<u8>, reserved_bytes: u64) -> Self {
        Self {
            data,
            reserved_bytes,
        }
    }

    fn reserved_bytes(&self) -> u64 {
        self.reserved_bytes
    }
}

impl std::fmt::Debug for MemoryGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryGuard")
            .field("len", &self.data.len())
            .finish()
    }
}

impl AsRef<[u8]> for MemoryGuard {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

/// Delegates to the centralized platform module for advisory file locking.
fn try_shared_lock(file: &File) -> bool {
    super::super::platform::try_shared_lock(file)
}

/// Image source - supports both in-memory data and memory-mapped files
#[derive(Clone, Debug)]
pub enum Source {
    /// In-memory image data (from Buffer)
    Memory(Arc<MemoryGuard>),
    /// Memory-mapped file with fd retention for advisory lock persistence
    Mapped(Arc<MmapGuard>),
}

impl Source {
    /// Create an in-memory source from owned bytes.
    pub fn from_vec(data: Vec<u8>) -> Self {
        Self::Memory(Arc::new(MemoryGuard::new(data)))
    }

    /// Get the bytes directly - zero-copy for both Memory and Mapped sources
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Source::Memory(data) => Some(data.as_ref().as_ref()),
            Source::Mapped(guard) => Some(guard.as_ref()),
        }
    }

    /// Get the length of the source data
    pub fn len(&self) -> usize {
        match self {
            Source::Memory(data) => data.as_ref().as_ref().len(),
            Source::Mapped(guard) => guard.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Bytes already reserved in the weighted semaphore for this source.
    pub fn reserved_memory_bytes(&self) -> u64 {
        match self {
            Source::Memory(data) => data.reserved_bytes(),
            Source::Mapped(_) => 0,
        }
    }
}

/// Load a file safely with SIGBUS prevention.
///
/// **Strategy:**
/// - Files <= 256 MB: read entirely into memory (no SIGBUS risk)
/// - Files > 256 MB: mmap with advisory lock + fd retention
///
/// If an advisory lock cannot be acquired on a large file (another process
/// holds an exclusive lock), the file is read into memory as a fallback.
#[allow(dead_code)] // Used by api.rs/tasks.rs when napi feature is enabled
pub fn load_file_safe(path: &Path) -> Result<Source, LazyImageError> {
    let path_str = path.to_string_lossy();

    let mut file = File::open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            LazyImageError::file_not_found(path_str.to_string())
        } else {
            LazyImageError::file_read_failed(path_str.to_string(), e)
        }
    })?;

    let metadata = file
        .metadata()
        .map_err(|e| LazyImageError::file_read_failed(path_str.to_string(), e))?;

    let file_size = metadata.len();

    if file_size <= MMAP_SIZE_THRESHOLD {
        return load_open_file_into_memory(&mut file, file_size, path_str.as_ref());
    }

    // Large files: attempt mmap with advisory lock
    if !try_shared_lock(&file) {
        return load_open_file_into_memory(&mut file, file_size, path_str.as_ref());
    }

    // SAFETY: We hold an advisory shared lock on the file descriptor.
    // Cooperative writers that respect flock will not modify the file.
    // The fd is retained in MmapGuard so the lock persists until Source is dropped.
    let mmap = unsafe {
        Mmap::map(&file).map_err(|e| LazyImageError::mmap_failed(path_str.to_string(), e))?
    };

    Ok(Source::Mapped(Arc::new(MmapGuard { mmap, _file: file })))
}

fn load_open_file_into_memory(
    file: &mut File,
    file_size: u64,
    path: &str,
) -> Result<Source, LazyImageError> {
    let sem = memory::memory_semaphore();
    let reserved_bytes = file_size.min(sem.capacity());
    // Bound the read_to_end heap spike, but do not retain the permit for the
    // entire Source lifetime. Processing re-acquires source bytes together
    // with decode/encode memory to avoid idle-source semaphore deadlocks.
    let _load_permit = sem.acquire(file_size);
    let mut data = Vec::new();
    if let Err(e) = file.read_to_end(&mut data) {
        return Err(LazyImageError::file_read_failed(path.to_string(), e));
    }
    Ok(Source::Memory(Arc::new(MemoryGuard::with_reserved_bytes(
        data,
        reserved_bytes,
    ))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn small_file_loaded_into_memory() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        let data = vec![0xFFu8; 1024]; // 1 KB
        tmp.write_all(&data).unwrap();
        tmp.flush().unwrap();

        let source = load_file_safe(tmp.path()).unwrap();
        assert!(
            matches!(source, Source::Memory(_)),
            "small file should be loaded into memory"
        );
        assert_eq!(source.len(), 1024);
    }

    #[test]
    fn nonexistent_file_returns_error() {
        let result = load_file_safe(Path::new("/tmp/nonexistent_lazy_image_test_file.xyz"));
        assert!(result.is_err());
    }

    #[test]
    fn source_memory_has_correct_bytes() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        let data = b"hello lazy-image";
        tmp.write_all(data).unwrap();
        tmp.flush().unwrap();

        let source = load_file_safe(tmp.path()).unwrap();
        assert_eq!(source.as_bytes().unwrap(), data);
    }
}
