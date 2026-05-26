// src/engine/tasks/atomic.rs
//
// Atomic file-write helper used by every task that emits to disk.

use crate::error::LazyImageError;

/// Write `data` to `output_path` atomically (temp-file + fsync + rename).
///
/// All file outputs go through this single helper to guarantee crash-safe
/// writes: a power loss or process kill mid-write leaves the destination
/// either untouched or fully written, never half-populated. The temp file
/// lives in the destination's parent directory so the final `persist()`
/// is a same-filesystem rename (atomic per POSIX).
///
/// Returns the number of bytes written as `u32`. The check fails for files
/// larger than 4 GiB, but no realistic image encode produces that much
/// output; callers that don't need the count simply discard the return value.
pub(super) fn write_encoded_output(
    output_path: &str,
    data: &[u8],
) -> std::result::Result<u32, LazyImageError> {
    use std::io::Write;
    use tempfile::NamedTempFile;

    let output_dir = std::path::Path::new(output_path).parent().ok_or_else(|| {
        LazyImageError::invalid_argument(
            "path",
            output_path.to_string(),
            "output path must include a parent directory",
        )
    })?;

    let mut temp_file = NamedTempFile::new_in(output_dir).map_err(|e| {
        LazyImageError::file_write_failed(output_dir.to_string_lossy().to_string(), e)
    })?;

    let temp_path = temp_file.path().to_path_buf();
    let bytes_written = data
        .len()
        .try_into()
        .map_err(|_| LazyImageError::internal_panic("file size exceeds 4GB limit (u32::MAX)"))?;
    temp_file
        .write_all(data)
        .map_err(|e| LazyImageError::file_write_failed(temp_path.display().to_string(), e))?;
    temp_file
        .as_file_mut()
        .sync_all()
        .map_err(|e| LazyImageError::file_write_failed(temp_path.display().to_string(), e))?;

    temp_file.persist(output_path).map_err(|e| {
        let io_error = std::io::Error::other(format!("failed to persist file: {e}"));
        LazyImageError::file_write_failed(output_path.to_string(), io_error)
    })?;

    Ok(bytes_written)
}
