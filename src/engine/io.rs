// src/engine/io.rs
//
// I/O operations: Source enum, file loading, and ICC profile extraction

use super::memory;
use crate::error::LazyImageError;
#[cfg(feature = "avif")]
use libavif_sys::*;
use memmap2::Mmap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

const MAX_ICC_SOURCE_BYTES: usize = 8 * 1024 * 1024; // Hard cap to keep fuzz inputs bounded without breaking large images

/// Files at or below this size are read into memory instead of mmap'd.
/// This eliminates SIGBUS/SIGSEGV risk for the vast majority of images.
/// Only files above this threshold use mmap with advisory lock protection.
const MMAP_SIZE_THRESHOLD: u64 = 256 * 1024 * 1024; // 256 MB

pub type IccExtractionResult = std::result::Result<Option<Vec<u8>>, LazyImageError>;

fn icc_decode_error(format: &str, reason: &str) -> LazyImageError {
    LazyImageError::decode_failed(format!("{format} ICC extraction failed: {reason}"))
}

fn icc_internal_panic(format: &str, reason: &str) -> LazyImageError {
    LazyImageError::internal_panic(format!("{format} ICC extraction panic: {reason}"))
}

// =============================================================================
// ICC EXTRACTOR TRAIT
// =============================================================================

/// Trait for format-specific ICC profile extraction.
///
/// Each image format stores ICC profiles differently (JPEG APP2 segments,
/// PNG iCCP chunks, WebP ICCP RIFF chunks, AVIF colr boxes). Implementors
/// parse their format's container to locate and return raw ICC profile bytes.
///
/// Validation of the extracted ICC data is handled centrally by
/// `extract_and_validate_icc()`, so implementors only need to handle
/// format-specific container parsing.
pub(crate) trait IccExtractor: std::panic::RefUnwindSafe {
    /// Human-readable format name for error messages (e.g. "jpeg", "png").
    fn format_name(&self) -> &'static str;

    /// Extract raw ICC profile bytes from format-specific container.
    /// Returns `None` when no ICC profile is present.
    fn extract_icc(&self, data: &[u8]) -> Option<Vec<u8>>;
}

/// Extract ICC profile using a format-specific extractor, with panic guard
/// and ICC header validation.
///
/// This is the single entry point that unifies the extraction + validation
/// flow across all formats. Format dispatch happens via the `IccExtractor`
/// trait, while this function owns the cross-cutting concerns:
/// 1. Panic guard (catch_unwind)
/// 2. ICC header validation (size, signature, version)
fn extract_and_validate_icc(extractor: &dyn IccExtractor, data: &[u8]) -> IccExtractionResult {
    let format = extractor.format_name();

    let icc_data = guard_icc_extraction(format, || Ok(extractor.extract_icc(data)))?;

    let Some(icc_data) = icc_data else {
        return Ok(None);
    };

    if !validate_icc_profile(&icc_data) {
        return Err(icc_decode_error(
            "icc",
            "invalid ICC header (size or signature mismatch)",
        ));
    }

    Ok(Some(icc_data))
}

pub(crate) struct JpegIccExtractor;
pub(crate) struct PngIccExtractor;
pub(crate) struct WebpIccExtractor;
pub(crate) struct AvifIccExtractor;

impl IccExtractor for JpegIccExtractor {
    fn format_name(&self) -> &'static str {
        "jpeg"
    }

    fn extract_icc(&self, data: &[u8]) -> Option<Vec<u8>> {
        extract_icc_from_jpeg(data)
    }
}

impl IccExtractor for PngIccExtractor {
    fn format_name(&self) -> &'static str {
        "png"
    }

    fn extract_icc(&self, data: &[u8]) -> Option<Vec<u8>> {
        extract_icc_from_png_direct(data)
    }
}

impl IccExtractor for WebpIccExtractor {
    fn format_name(&self) -> &'static str {
        "webp"
    }

    fn extract_icc(&self, data: &[u8]) -> Option<Vec<u8>> {
        extract_icc_from_webp_riff(data)
    }
}

impl IccExtractor for AvifIccExtractor {
    fn format_name(&self) -> &'static str {
        "avif"
    }

    fn extract_icc(&self, data: &[u8]) -> Option<Vec<u8>> {
        extract_icc_from_avif_safe(data)
    }
}

/// Select the appropriate ICC extractor based on image magic bytes.
fn select_icc_extractor(data: &[u8]) -> Option<&'static dyn IccExtractor> {
    if data.starts_with(&[0xFF, 0xD8]) {
        Some(&JpegIccExtractor)
    } else if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        Some(&PngIccExtractor)
    } else if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WEBP" {
        Some(&WebpIccExtractor)
    } else if is_avif_data(data) {
        Some(&AvifIccExtractor)
    } else {
        None
    }
}

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
    super::platform::try_shared_lock(file)
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

    // Safety: We hold an advisory shared lock on the file descriptor.
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

/// Extract ICC profile from image data.
/// Supports JPEG (APP2 marker), PNG (iCCP chunk), WebP (ICCP chunk), and AVIF (colr box).
/// Returns `Ok(None)` when no ICC profile is present or the format is unsupported.
/// Returns `Err` for structurally invalid containers or corrupted ICC payloads.
pub fn extract_icc_profile(data: &[u8]) -> IccExtractionResult {
    if data.len() < 12 {
        return Ok(None);
    }
    // For very large inputs, skip ICC parsing to avoid unbounded work but keep processing alive.
    if data.len() > MAX_ICC_SOURCE_BYTES {
        return Ok(None);
    }

    let Some(extractor) = select_icc_extractor(data) else {
        return Ok(None);
    };

    extract_and_validate_icc(extractor, data)
}

/// Lossy helper for callers that cannot propagate errors (legacy NAPI constructor).
/// Panics are still trapped and converted to `None` via the guard.
pub fn extract_icc_profile_lossy(data: &[u8]) -> Option<Vec<u8>> {
    guard_icc_extraction("lossy", || extract_icc_profile(data)).unwrap_or(None)
}

fn guard_icc_extraction<F>(format: &'static str, func: F) -> IccExtractionResult
where
    F: FnOnce() -> IccExtractionResult + std::panic::UnwindSafe,
{
    std::panic::catch_unwind(func)
        .map_err(|_| icc_internal_panic(format, "panic during ICC parse"))?
}

/// Validate ICC profile header
/// ICC profiles must start with a 128-byte header containing specific fields
pub(crate) fn validate_icc_profile(icc_data: &[u8]) -> bool {
    // Minimum ICC profile size is 128 bytes (header)
    if icc_data.len() < 128 {
        return false;
    }

    // Check profile size field (bytes 0-3, big-endian)
    let profile_size =
        u32::from_be_bytes([icc_data[0], icc_data[1], icc_data[2], icc_data[3]]) as usize;

    // Profile size must match actual data length
    if profile_size != icc_data.len() {
        return false;
    }

    // Check preferred CMM type (bytes 4-7) - should be ASCII
    // Common values: "ADBE", "appl", "lcms", etc.
    // We just check that it's printable ASCII
    for &byte in &icc_data[4..8] {
        if !(32..=126).contains(&byte) && byte != 0 {
            return false;
        }
    }

    // Check profile version (bytes 8-11)
    // Major version should be reasonable (typically 2, 4, or 5)
    let major_version = icc_data[8];
    if major_version > 10 {
        return false;
    }

    // Check profile class signature (bytes 12-15)
    // Common: "mntr" (monitor), "prtr" (printer), "scnr" (scanner), "spac" (color space)
    // We just check that it's ASCII
    for &byte in &icc_data[12..16] {
        if !(32..=126).contains(&byte) && byte != 0 {
            return false;
        }
    }

    // Check data color space (bytes 16-19) - should be ASCII
    for &byte in &icc_data[16..20] {
        if !(32..=126).contains(&byte) && byte != 0 {
            return false;
        }
    }

    // Check PCS (Profile Connection Space) signature (bytes 20-23) - should be ASCII
    for &byte in &icc_data[20..24] {
        if !(32..=126).contains(&byte) && byte != 0 {
            return false;
        }
    }

    // Basic validation passed
    true
}

/// Check if data is AVIF format (ISOBMFF with 'avif' brand)
pub(crate) fn is_avif_data(data: &[u8]) -> bool {
    // AVIF files are ISOBMFF containers
    // They start with a 'ftyp' box containing 'avif' or 'avis' brand
    if data.len() < 12 {
        return false;
    }

    // Check for 'ftyp' box (first 4 bytes are size, next 4 are 'ftyp')
    if &data[4..8] != b"ftyp" {
        return false;
    }

    // Look for 'avif' or 'avis' brand in ftyp box
    let ftyp_size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if ftyp_size > data.len() || ftyp_size < 12 {
        return false;
    }

    // Check major brand (bytes 8-11)
    let major_brand = &data[8..12];
    if major_brand == b"avif" || major_brand == b"avis" {
        return true;
    }

    // Check compatible brands (starting at byte 16)
    if ftyp_size >= 20 {
        let mut offset = 16;
        while offset + 4 <= ftyp_size {
            let brand = &data[offset..offset + 4];
            if brand == b"avif" || brand == b"avis" {
                return true;
            }
            offset += 4;
        }
    }

    false
}

/// Extract ICC profile from JPEG data using a guarded APP2 parser.
///
/// Validates JPEG structure first, then parses APP2 segments following
/// the ICC.1 multi-segment spec.
pub(crate) fn extract_icc_from_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    // Reuse the canonical JPEG structure validator from the decoder module.
    // If the structure is invalid, skip ICC extraction entirely.
    if crate::engine::decoder::validate_jpeg_structure(data).is_err() {
        return None;
    }
    extract_icc_from_jpeg_app2(data)
}

/// Extract ICC profile from APP2 segments following the ICC.1 spec.
fn extract_icc_from_jpeg_app2(data: &[u8]) -> Option<Vec<u8>> {
    const APP2: u8 = 0xE2;
    const SOS: u8 = 0xDA;
    const EOI: u8 = 0xD9;
    const ICC_ID: &[u8] = b"ICC_PROFILE\0";

    let mut i = 2; // skip SOI
    let mut count_expected: Option<u8> = None;
    let mut segments: Vec<(u8, Vec<u8>)> = Vec::new();

    while i + 1 < data.len() {
        if data[i] != 0xFF {
            return None;
        }
        while i < data.len() && data[i] == 0xFF {
            i += 1;
        }
        if i >= data.len() {
            return None;
        }
        let marker = data[i];
        i += 1;

        if marker == SOS || marker == EOI {
            break; // stop before compressed scan or explicit end
        }
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            continue; // standalone marker
        }

        if i + 1 >= data.len() {
            return None;
        }
        let seg_len = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
        if seg_len < 2 || i + seg_len > data.len() {
            return None;
        }
        i += 2;
        let seg_end = i + seg_len - 2;
        if seg_end > data.len() {
            return None;
        }

        if marker == APP2 && seg_len >= 16 {
            let segment = &data[i..seg_end];
            if segment.len() >= 14 && segment.starts_with(ICC_ID) {
                let seq = segment[12];
                let count = segment[13];
                if seq == 0 || count == 0 {
                    // invalid numbering, skip
                } else {
                    if let Some(expected) = count_expected {
                        if expected != count {
                            return None;
                        }
                    } else {
                        count_expected = Some(count);
                    }
                    let payload = segment[14..].to_vec();
                    segments.push((seq, payload));
                }
            }
        }

        i = seg_end;
    }

    let total_segments = count_expected?;
    if segments.is_empty() || segments.len() != total_segments as usize {
        return None;
    }

    segments.sort_by_key(|(seq, _)| *seq);
    for (expected_seq, (seq, _)) in (1u8..=total_segments).zip(segments.iter()) {
        if *seq != expected_seq {
            return None;
        }
    }

    let total_len: usize = segments.iter().map(|(_, payload)| payload.len()).sum();
    if total_len > MAX_ICC_SOURCE_BYTES {
        return None;
    }

    let mut combined = Vec::with_capacity(total_len);
    for (_, payload) in segments {
        combined.extend_from_slice(&payload);
    }
    Some(combined)
}

/// Extract ICC profile from PNG iCCP chunk by directly parsing PNG structure.
///
/// Parses the PNG chunk stream to find iCCP, then zlib-decompresses
/// the embedded ICC profile data.
pub(crate) fn extract_icc_from_png_direct(data: &[u8]) -> Option<Vec<u8>> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    // PNG signature: 0x89 0x50 0x4E 0x47 0x0D 0x0A 0x1A 0x0A
    if data.len() < 8 || data[0..8] != [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return None;
    }

    let mut offset = 8; // Skip PNG signature

    // Parse chunks
    while offset + 8 <= data.len() {
        // Read chunk length (4 bytes, big-endian)
        let chunk_length = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;
        offset += 4;

        // Read chunk type (4 bytes)
        if offset + 4 > data.len() {
            break;
        }
        let chunk_type = &data[offset..offset + 4];
        offset += 4;

        // Check if this is an iCCP chunk
        if chunk_type == b"iCCP" {
            // iCCP chunk format: profile_name (null-terminated) + compression_method (1 byte) + compressed_data
            if offset + chunk_length > data.len() {
                break;
            }

            // Find null terminator for profile name
            let mut name_end = offset;
            while name_end < offset + chunk_length && data[name_end] != 0 {
                name_end += 1;
            }
            if name_end >= offset + chunk_length {
                break;
            }

            // Skip profile name and null terminator
            let data_start = name_end + 1;
            if data_start >= offset + chunk_length {
                break;
            }

            // Read compression method (should be 0 = zlib)
            let compression_method = data[data_start];
            if compression_method != 0 {
                break;
            }

            // Read compressed data
            let compressed_start = data_start + 1;
            let compressed_end = offset + chunk_length;
            if compressed_start >= compressed_end {
                break;
            }
            let compressed_data = &data[compressed_start..compressed_end];

            // Decompress zlib data
            let mut decoder = ZlibDecoder::new(compressed_data);
            let mut decompressed = Vec::new();
            if decoder.read_to_end(&mut decompressed).is_ok() {
                return Some(decompressed);
            }
            break;
        }

        // Skip chunk data and CRC (4 bytes)
        offset += chunk_length + 4;
    }

    None
}

/// Extract ICC profile from WebP RIFF container by walking RIFF chunks
/// and reading the ICCP chunk payload.
fn extract_icc_from_webp_riff(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 12 || !data.starts_with(b"RIFF") || &data[8..12] != b"WEBP" {
        return None;
    }

    // RIFF chunk size is bytes 4..8 (little-endian) but we only validate bounds defensively.
    let mut offset = 12;
    while offset + 8 <= data.len() {
        let chunk_id = &data[offset..offset + 4];
        let size = u32::from_le_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize;
        offset = offset
            .checked_add(8)
            .filter(|&v| v <= data.len())
            .unwrap_or(data.len());

        if offset + size > data.len() {
            break;
        }

        if chunk_id == b"ICCP" {
            return Some(data[offset..offset + size].to_vec());
        }

        // Chunks are padded to even sizes.
        let padded = if size.is_multiple_of(2) {
            size
        } else {
            size + 1
        };
        offset = offset.saturating_add(padded);
    }

    None
}

/// Extract ICC profile from AVIF data using libavif with panic and size guards.
/// Enabled only when the `avif` feature is active.
#[cfg(feature = "avif")]
fn extract_icc_from_avif_safe(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() > MAX_ICC_SOURCE_BYTES {
        return None;
    }

    std::panic::catch_unwind(|| unsafe {
        // Create decoder
        let decoder = avifDecoderCreate();
        if decoder.is_null() {
            return None;
        }

        // Set up RAII cleanup
        struct AvifDecoderGuard(*mut avifDecoder);
        impl Drop for AvifDecoderGuard {
            fn drop(&mut self) {
                unsafe {
                    if !self.0.is_null() {
                        avifDecoderDestroy(self.0);
                    }
                }
            }
        }
        let _decoder_guard = AvifDecoderGuard(decoder);

        // Keep fuzz runs single-threaded to reduce nondeterminism and memory overhead.
        (*decoder).maxThreads = 1;

        // Set decode data
        let result = avifDecoderSetIOMemory(decoder, data.as_ptr(), data.len());
        if result != AVIF_RESULT_OK {
            return None;
        }

        // Parse the image (header only)
        let result = avifDecoderParse(decoder);
        if result != AVIF_RESULT_OK {
            return None;
        }

        // Get the image
        let image = (*decoder).image;
        if image.is_null() {
            return None;
        }

        // Check if ICC profile exists
        let icc_size = (*image).icc.size;
        if icc_size == 0 {
            return None;
        }

        // Copy ICC profile data
        let icc_ptr = (*image).icc.data;
        if icc_ptr.is_null() {
            return None;
        }

        if icc_size > MAX_ICC_SOURCE_BYTES {
            return None;
        }

        let icc_data = std::slice::from_raw_parts(icc_ptr, icc_size).to_vec();
        Some(icc_data)
    })
    .ok()
    .flatten()
}

#[cfg(not(feature = "avif"))]
fn extract_icc_from_avif_safe(_data: &[u8]) -> Option<Vec<u8>> {
    None
}

// =============================================================================
// EXIF METADATA EXTRACTION AND SANITIZATION
// =============================================================================
//
// Uses little_exif for tag-level EXIF access, enabling:
// - Orientation tag reset after auto-orient (prevents double-rotation bugs)
// - GPS tag stripping by default (privacy-first, exceeds Sharp's capabilities)

#[allow(unused_imports)]
use little_exif::filetype::FileExtension;

/// Maximum EXIF data size to process (prevent DoS from malicious inputs)
const MAX_EXIF_SOURCE_BYTES: usize = 8 * 1024 * 1024;

/// Detect file extension for little_exif from image data magic bytes.
/// Note: Reserved for future PNG/WebP/AVIF EXIF support.
#[allow(dead_code)]
fn detect_file_extension(data: &[u8]) -> Option<FileExtension> {
    if data.len() < 12 {
        return None;
    }

    // JPEG: starts with FF D8
    if data.starts_with(&[0xFF, 0xD8]) {
        return Some(FileExtension::JPEG);
    }

    // PNG: starts with 89 50 4E 47
    // as_zTXt_chunk: true means EXIF is stored in zTXt chunk (standard for PNG)
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Some(FileExtension::PNG {
            as_zTXt_chunk: true,
        });
    }

    // WebP: RIFF....WEBP
    if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WEBP" {
        return Some(FileExtension::WEBP);
    }

    // AVIF/HEIF: ISOBMFF with ftyp box - little_exif uses HEIF for both
    if &data[4..8] == b"ftyp" {
        let brand = &data[8..12];
        if brand == b"avif"
            || brand == b"avis"
            || brand == b"heic"
            || brand == b"heix"
            || brand == b"mif1"
        {
            return Some(FileExtension::HEIF);
        }
    }

    // TIFF: II (little-endian) or MM (big-endian)
    if (data.starts_with(b"II") && data[2] == 0x2A && data[3] == 0x00)
        || (data.starts_with(b"MM") && data[2] == 0x00 && data[3] == 0x2A)
    {
        return Some(FileExtension::TIFF);
    }

    None
}

/// Extract raw EXIF bytes from image data.
/// This is used to store EXIF for later embedding without needing to re-parse.
/// Returns the raw EXIF APP1 segment data (for JPEG) or equivalent for other formats.
///
/// Gated by `MAX_EXIF_SOURCE_BYTES` because the result is copied onto the heap.
/// For presence checks that should not be size-gated (e.g. metrics), use
/// [`has_exif`] instead.
pub fn extract_exif_raw(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 12 || data.len() > MAX_EXIF_SOURCE_BYTES {
        return None;
    }

    // For JPEG, extract raw APP1 segment containing EXIF
    if data.starts_with(&[0xFF, 0xD8]) {
        return find_exif_segment_jpeg(data).map(<[u8]>::to_vec);
    }

    // EXIF extraction for non-JPEG formats is not yet implemented.
    // Returning None prevents the entire file buffer from being stored as "EXIF data",
    // which wastes memory and can bypass firewall metadata size limits.
    // JPEG EXIF extraction is handled above via find_exif_segment_jpeg().
    None
}

/// Lightweight EXIF-presence check.
///
/// Unlike [`extract_exif_raw`], this is **not** gated by `MAX_EXIF_SOURCE_BYTES`
/// and performs zero heap allocations — it only walks the JPEG segment list
/// looking for an `Exif\0\0` APP1 marker.
///
/// Use this in code paths (e.g. metrics finalization) that need to know whether
/// source EXIF existed without paying the cost of extracting it. Large (>8 MiB)
/// JPEGs with APP1 EXIF will be reported correctly here, whereas
/// `extract_exif_raw` would return `None` for them.
pub fn has_exif(data: &[u8]) -> bool {
    if data.len() < 12 {
        return false;
    }

    if data.starts_with(&[0xFF, 0xD8]) {
        return find_exif_segment_jpeg(data).is_some();
    }

    // Mirror `extract_exif_raw`: non-JPEG EXIF support is not implemented yet.
    false
}

/// Detect whether a TIFF/EXIF payload (the bytes following the `Exif\0\0`
/// identifier in a JPEG APP1 segment) contains a GPS IFD pointer (tag 0x8825)
/// in IFD0.
///
/// Returns `false` on any parse error — never falsely claims GPS is present.
/// This is conservative on purpose: callers use the result to decide whether
/// "GPS was stripped" should be reported in metrics, and a false positive would
/// claim metadata was stripped when nothing changed.
pub fn exif_has_gps(exif: &[u8]) -> bool {
    // TIFF header: 2-byte byte-order + 2-byte magic (0x002A) + 4-byte IFD0 offset.
    if exif.len() < 8 {
        return false;
    }

    let is_little_endian = match &exif[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return false,
    };
    let read_u16 = |b: [u8; 2]| {
        if is_little_endian {
            u16::from_le_bytes(b)
        } else {
            u16::from_be_bytes(b)
        }
    };
    let read_u32 = |b: [u8; 4]| {
        if is_little_endian {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        }
    };

    let magic = read_u16([exif[2], exif[3]]);
    if magic != 0x002A {
        return false;
    }

    let ifd0_offset = read_u32([exif[4], exif[5], exif[6], exif[7]]) as usize;
    if ifd0_offset < 8 || ifd0_offset + 2 > exif.len() {
        return false;
    }

    let num_entries = read_u16([exif[ifd0_offset], exif[ifd0_offset + 1]]) as usize;
    const ENTRY_SIZE: usize = 12;
    const GPS_IFD_POINTER_TAG: u16 = 0x8825;

    let entries_start = ifd0_offset + 2;
    let entries_bytes = match num_entries.checked_mul(ENTRY_SIZE) {
        Some(n) => n,
        None => return false,
    };
    let entries_end = match entries_start.checked_add(entries_bytes) {
        Some(end) => end,
        None => return false,
    };
    if entries_end > exif.len() {
        return false;
    }

    for n in 0..num_entries {
        let off = entries_start + n * ENTRY_SIZE;
        if read_u16([exif[off], exif[off + 1]]) == GPS_IFD_POINTER_TAG {
            return true;
        }
    }
    false
}

/// Walk JPEG markers and return a borrowed slice over the EXIF payload (the
/// bytes following the `Exif\0\0` identifier).
///
/// Allocation-free. Returns `None` when the input is not a JPEG, no EXIF APP1
/// segment is present, or the segment structure is malformed.
fn find_exif_segment_jpeg(data: &[u8]) -> Option<&[u8]> {
    const APP1: u8 = 0xE1;
    const SOS: u8 = 0xDA;
    const EOI: u8 = 0xD9;
    const EXIF_ID: &[u8] = b"Exif\0\0";

    if !data.starts_with(&[0xFF, 0xD8]) {
        return None;
    }

    let mut i = 2; // skip SOI

    while i + 1 < data.len() {
        if data[i] != 0xFF {
            return None;
        }
        while i < data.len() && data[i] == 0xFF {
            i += 1;
        }
        if i >= data.len() {
            return None;
        }
        let marker = data[i];
        i += 1;

        if marker == SOS || marker == EOI {
            break; // stop before compressed scan or explicit end
        }
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            continue; // standalone marker
        }

        if i + 1 >= data.len() {
            return None;
        }
        let seg_len = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
        if seg_len < 2 || i + seg_len > data.len() {
            return None;
        }
        i += 2;
        let seg_end = i + seg_len - 2;
        if seg_end > data.len() {
            return None;
        }

        // Check for EXIF APP1 segment
        if marker == APP1 && seg_len >= 8 {
            let segment = &data[i..seg_end];
            if segment.starts_with(EXIF_ID) {
                // Return the EXIF payload borrowed from the input buffer
                // (without the `Exif\0\0` identifier).
                return Some(&segment[6..]);
            }
        }

        i = seg_end;
    }

    None
}

/// Test-only re-export so unit tests can exercise the borrowed-slice helper
/// directly without paying for the heap allocation that `extract_exif_raw`
/// performs.
#[cfg(test)]
pub(crate) fn find_exif_segment_jpeg_for_tests(data: &[u8]) -> Option<&[u8]> {
    find_exif_segment_jpeg(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "avif")]
    use crate::engine::encoder::encode_avif;
    use crate::engine::encoder::{encode_jpeg, encode_png, encode_webp};
    use image::{DynamicImage, RgbImage};
    use std::io::Cursor;

    fn extract_icc_ok(data: &[u8]) -> Option<Vec<u8>> {
        extract_icc_profile(data).unwrap()
    }

    // Helper function to create test images
    fn create_test_image(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        }))
    }

    // Helper to create minimal valid JPEG bytes
    fn create_minimal_jpeg() -> Vec<u8> {
        // Create a 1x1 RGB image and encode it as JPEG
        let img = create_test_image(1, 1);
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let pixels = rgb.into_raw();

        // Use mozjpeg to create a valid JPEG
        let mut comp = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
        comp.set_size(w as usize, h as usize);
        comp.set_quality(80.0);
        comp.set_color_space(mozjpeg::ColorSpace::JCS_YCbCr);
        comp.set_chroma_sampling_pixel_sizes((2, 2), (2, 2));

        let mut output = Vec::new();
        {
            let mut writer = comp.start_compress(&mut output).unwrap();
            let stride = w as usize * 3;
            for row in pixels.chunks(stride) {
                writer.write_scanlines(row).unwrap();
            }
            writer.finish().unwrap();
        }
        output
    }

    // Helper to create minimal valid PNG bytes
    fn create_minimal_png() -> Vec<u8> {
        let img = create_test_image(1, 1);
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    // Helper to create minimal valid WebP bytes
    fn create_minimal_webp() -> Vec<u8> {
        let img = create_test_image(10, 10);
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let encoder = webp::Encoder::from_rgb(&rgb, w, h);
        let config = webp::WebPConfig::new().unwrap();
        let mem = encoder.encode_advanced(&config).unwrap();
        mem.to_vec()
    }

    mod icc_tests {
        use super::*;

        #[test]
        fn test_validate_icc_profile_too_small() {
            let data = vec![0u8; 127]; // Less than 128 bytes
            assert!(!validate_icc_profile(&data));
        }

        #[test]
        fn test_validate_icc_profile_minimal_valid() {
            // Minimal valid ICC profile (128 bytes)
            let mut data = vec![0u8; 128];
            // Profile size (first 4 bytes, big-endian)
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0x80; // 128 bytes
                            // CMM type (bytes 4-7): "ADBE" (ASCII)
            data[4] = b'A';
            data[5] = b'D';
            data[6] = b'B';
            data[7] = b'E';
            // Version (byte 8): 2
            data[8] = 2;
            // Profile class (bytes 12-15): "mntr" (monitor)
            data[12] = b'm';
            data[13] = b'n';
            data[14] = b't';
            data[15] = b'r';
            // Data color space (bytes 16-19): "RGB " (ASCII)
            data[16] = b'R';
            data[17] = b'G';
            data[18] = b'B';
            data[19] = b' ';
            // PCS (bytes 20-23): "XYZ " (ASCII)
            data[20] = b'X';
            data[21] = b'Y';
            data[22] = b'Z';
            data[23] = b' ';

            assert!(validate_icc_profile(&data));
        }

        #[test]
        fn test_validate_icc_profile_size_mismatch() {
            let mut data = vec![0u8; 200];
            // Set profile size to 200
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0xC8; // 200 bytes
                            // But actual data is 200 bytes, so this is valid
                            // Test case where size doesn't match
            data[3] = 0x00;
            data[3] = 0xFF; // Set to 255 bytes (actual is 200 bytes)

            // Invalid because size doesn't match
            assert!(!validate_icc_profile(&data));
        }

        #[test]
        fn test_validate_icc_profile_invalid_version() {
            let mut data = vec![0u8; 128];
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0x80;
            data[8] = 20; // Version too large

            assert!(!validate_icc_profile(&data));
        }

        #[test]
        fn test_extract_icc_from_jpeg_no_profile() {
            // JPEG without ICC profile
            let jpeg_data = create_minimal_jpeg();
            let result = JpegIccExtractor.extract_icc(&jpeg_data);
            assert!(result.is_none());
        }

        #[test]
        fn test_extract_icc_from_png_no_profile() {
            // PNG without ICC profile
            let png_data = create_minimal_png();
            let result = PngIccExtractor.extract_icc(&png_data);
            assert!(result.is_none());
        }

        #[test]
        fn test_extract_icc_from_webp_no_profile() {
            // WebP without ICC profile
            let webp_data = create_minimal_webp();
            let result = WebpIccExtractor.extract_icc(&webp_data);
            assert!(result.is_none());
        }

        #[test]
        fn test_extract_icc_profile_invalid_data() {
            let invalid_data = vec![0u8; 10];
            let result = extract_icc_profile(&invalid_data);
            assert!(result.is_ok());
            assert!(result.unwrap().is_none());
        }

        #[test]
        fn test_extract_icc_profile_large_input_skips_icc() {
            // Ensure inputs larger than the fuzz safety cap do not hard-fail.
            let huge = vec![0u8; MAX_ICC_SOURCE_BYTES + 1];
            let result = extract_icc_profile(&huge);
            assert!(result.is_ok());
            assert!(result.unwrap().is_none());
        }

        #[test]
        fn test_extract_icc_profile_jpeg() {
            let jpeg_data = create_minimal_jpeg();
            // Extract ICC profile from JPEG (when not present)
            let result = extract_icc_profile(&jpeg_data);
            // Minimal JPEG has no ICC profile
            assert!(result.is_ok());
            assert!(result.unwrap().is_none());
        }

        // Helper function to create a minimal valid ICC profile (sRGB)
        fn create_minimal_srgb_icc() -> Vec<u8> {
            // Minimal valid sRGB ICC profile (128 bytes)
            let mut data = vec![0u8; 128];
            // Profile size (first 4 bytes, big-endian)
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0x80; // 128 bytes
                            // CMM type (bytes 4-7): "ADBE" (ASCII)
            data[4] = b'A';
            data[5] = b'D';
            data[6] = b'B';
            data[7] = b'E';
            // Version (byte 8): 2
            data[8] = 2;
            // Profile class (bytes 12-15): "mntr" (monitor)
            data[12] = b'm';
            data[13] = b'n';
            data[14] = b't';
            data[15] = b'r';
            // Data color space (bytes 16-19): "RGB " (ASCII)
            data[16] = b'R';
            data[17] = b'G';
            data[18] = b'B';
            data[19] = b' ';
            // PCS (bytes 20-23): "XYZ " (ASCII)
            data[20] = b'X';
            data[21] = b'Y';
            data[22] = b'Z';
            data[23] = b' ';
            data
        }

        // Helper function to create JPEG with ICC profile
        fn create_jpeg_with_icc(icc: &[u8]) -> Vec<u8> {
            let img = create_test_image(100, 100);
            encode_jpeg(&img, 80, Some(icc)).unwrap()
        }

        // Helper function to create PNG with ICC profile
        fn create_png_with_icc(icc: &[u8]) -> Vec<u8> {
            let img = create_test_image(100, 100);
            encode_png(&img, Some(icc)).unwrap()
        }

        // Helper function to create WebP with ICC profile
        fn create_webp_with_icc(icc: &[u8]) -> Vec<u8> {
            let img = create_test_image(100, 100);
            encode_webp(&img, 80, Some(icc)).unwrap()
        }

        mod extraction_tests {
            use super::*;

            #[test]
            fn test_extract_icc_from_jpeg_with_profile() {
                let icc = create_minimal_srgb_icc();
                let jpeg = create_jpeg_with_icc(&icc);
                let extracted = extract_icc_ok(&jpeg);
                assert!(extracted.is_some());
                let extracted = extracted.unwrap();
                // Minimum ICC profile size is 128 bytes (header)
                assert!(extracted.len() >= 128);
            }

            #[test]
            fn test_extract_icc_from_png_with_profile() {
                // PNG ICC extraction should succeed for valid iCCP chunks.
                // The direct parser fallback keeps this deterministic across environments.
                let icc = create_minimal_srgb_icc();
                let png = create_png_with_icc(&icc);
                let extracted = extract_icc_ok(&png);
                // PNG ICC extraction should return Some when ICC profile is embedded correctly
                assert!(
                    extracted.is_some(),
                    "PNG ICC extraction should return Some when ICC profile is embedded correctly"
                );
                let extracted = extracted.unwrap();
                // Minimum ICC profile size is 128 bytes (header)
                assert!(extracted.len() >= 128);
                // Extracted ICC should match original
                assert_eq!(icc, extracted, "Extracted ICC should match original");
            }

            #[test]
            fn test_extract_icc_from_webp_with_profile() {
                let icc = create_minimal_srgb_icc();
                let webp = create_webp_with_icc(&icc);
                let extracted = extract_icc_ok(&webp);
                assert!(extracted.is_some());
            }

            #[test]
            fn test_extract_icc_returns_none_for_no_icc() {
                let jpeg = create_minimal_jpeg();
                let icc = extract_icc_ok(&jpeg);
                assert!(icc.is_none());
            }

            #[test]
            fn test_extract_icc_returns_none_for_non_image() {
                let icc = extract_icc_ok(b"not an image");
                assert!(icc.is_none());
            }

            #[test]
            fn test_extract_icc_returns_none_for_empty() {
                let icc = extract_icc_ok(&[]);
                assert!(icc.is_none());
            }
        }

        mod validation_tests {
            use super::*;

            #[test]
            fn test_validate_valid_icc() {
                let icc = create_minimal_srgb_icc();
                assert!(validate_icc_profile(&icc));
            }

            #[test]
            fn test_validate_truncated_icc() {
                let icc = create_minimal_srgb_icc();
                // Truncated in the middle
                let truncated = &icc[..50];
                assert!(!validate_icc_profile(truncated));
            }

            #[test]
            fn test_validate_wrong_size_field() {
                let mut icc = create_minimal_srgb_icc();
                // Set size field (first 4 bytes) to invalid value
                icc[0] = 0xFF;
                icc[1] = 0xFF;
                icc[2] = 0xFF;
                icc[3] = 0xFF;
                assert!(!validate_icc_profile(&icc));
            }

            #[test]
            fn test_validate_too_short() {
                assert!(!validate_icc_profile(&[0; 100])); // Less than 128 bytes
            }

            #[test]
            fn test_validate_empty() {
                assert!(!validate_icc_profile(&[]));
            }
        }

        mod roundtrip_tests {
            use super::*;

            #[test]
            fn test_jpeg_roundtrip() {
                // 1. Extract ICC from original image
                let original_icc = create_minimal_srgb_icc();
                let jpeg = create_jpeg_with_icc(&original_icc);
                let extracted_icc = extract_icc_ok(&jpeg).unwrap();

                // 2. Decode image
                let img = image::load_from_memory(&jpeg).unwrap();

                // 3. Encode JPEG with ICC embedded
                let encoded = encode_jpeg(&img, 80, Some(&extracted_icc)).unwrap();

                // 4. Re-extract ICC from encoded result
                let re_extracted_icc = extract_icc_ok(&encoded).unwrap();

                // 5. Verify identity
                assert_eq!(extracted_icc, re_extracted_icc);
            }

            #[test]
            fn test_png_roundtrip() {
                // Test that ICC profile is preserved in PNG roundtrip
                let original_icc = create_minimal_srgb_icc();
                let png = create_png_with_icc(&original_icc);

                // Verify that iCCP chunk exists in PNG (using direct parsing)
                let extracted_icc = extract_icc_from_png_direct(&png);
                assert!(
                    extracted_icc.is_some(),
                    "PNG should contain iCCP chunk with ICC profile"
                );
                let extracted_icc = extracted_icc.unwrap();
                assert_eq!(
                    original_icc, extracted_icc,
                    "Extracted ICC should match original"
                );

                // Test roundtrip: decode and re-encode
                let img = image::load_from_memory(&png).unwrap();
                let encoded = encode_png(&img, Some(&extracted_icc)).unwrap();

                // Verify that re-encoded PNG also contains iCCP chunk
                let re_extracted_icc = extract_icc_from_png_direct(&encoded);
                assert!(
                    re_extracted_icc.is_some(),
                    "Re-encoded PNG should also contain iCCP chunk"
                );
                assert_eq!(
                    extracted_icc,
                    re_extracted_icc.unwrap(),
                    "Re-extracted ICC should match original"
                );
            }

            #[test]
            fn test_webp_roundtrip() {
                let original_icc = create_minimal_srgb_icc();
                let webp = create_webp_with_icc(&original_icc);
                let extracted_icc = extract_icc_ok(&webp).unwrap();

                let img = image::load_from_memory(&webp).unwrap();
                let encoded = encode_webp(&img, 80, Some(&extracted_icc)).unwrap();
                let re_extracted_icc = extract_icc_ok(&encoded).unwrap();

                assert_eq!(extracted_icc, re_extracted_icc);
            }

            #[test]
            fn test_cross_format_roundtrip_jpeg_to_png() {
                // Test that ICC profile is preserved when converting JPEG to PNG
                let icc = create_minimal_srgb_icc();
                let jpeg = create_jpeg_with_icc(&icc);
                let extracted_icc = extract_icc_ok(&jpeg).unwrap();

                // Convert JPEG to PNG with ICC
                let img = image::load_from_memory(&jpeg).unwrap();
                let png = encode_png(&img, Some(&extracted_icc)).unwrap();

                // Verify that PNG contains iCCP chunk with ICC profile (using direct parsing)
                let re_extracted = extract_icc_from_png_direct(&png);
                assert!(
                    re_extracted.is_some(),
                    "PNG should contain iCCP chunk with ICC profile from JPEG"
                );
                assert_eq!(
                    extracted_icc,
                    re_extracted.unwrap(),
                    "ICC profile should be preserved in JPEG to PNG conversion"
                );
            }

            #[test]
            fn test_cross_format_roundtrip_png_to_webp() {
                // Test that ICC profile is preserved when converting PNG to WebP
                // Use direct parsing to assert the iCCP chunk deterministically
                let icc = create_minimal_srgb_icc();
                let png = create_png_with_icc(&icc);

                // Extract ICC from PNG using direct parsing
                let extracted_icc = extract_icc_from_png_direct(&png);
                assert!(
                    extracted_icc.is_some(),
                    "PNG should contain iCCP chunk with ICC profile"
                );
                let extracted_icc = extracted_icc.unwrap();
                assert_eq!(
                    icc, extracted_icc,
                    "Extracted ICC from PNG should match original"
                );

                // Convert PNG to WebP using extracted ICC
                let img = image::load_from_memory(&png).unwrap();
                let webp = encode_webp(&img, 80, Some(&extracted_icc)).unwrap();

                // Verify that WebP contains ICC profile
                let re_extracted = extract_icc_ok(&webp).unwrap();
                assert_eq!(
                    extracted_icc, re_extracted,
                    "ICC profile should be preserved in PNG to WebP conversion"
                );
            }
        }

        #[cfg(feature = "avif")]
        mod avif_icc_tests {
            use super::*;

            #[test]
            fn test_avif_preserves_icc_profile() {
                // libavif implementation now properly embeds ICC profiles
                // libavif-sys is always available (not dependent on napi feature)
                let icc = create_minimal_srgb_icc();
                let img = create_test_image(100, 100);
                let avif = encode_avif(&img, 60, Some(&icc)).unwrap();

                // Verify AVIF data is valid
                assert!(is_avif_data(&avif), "Output should be valid AVIF");

                // Extract ICC profile from AVIF
                let extracted = extract_icc_ok(&avif);
                assert!(
                    extracted.is_some(),
                    "AVIF should now preserve ICC profile with libavif"
                );

                // Verify extracted ICC matches original
                let extracted_icc = extracted.unwrap();
                assert_eq!(
                    extracted_icc.len(),
                    icc.len(),
                    "Extracted ICC size should match original"
                );
                assert_eq!(
                    &extracted_icc[..],
                    &icc[..],
                    "Extracted ICC data should match original"
                );
            }

            #[test]
            fn test_avif_encoding_with_icc_does_not_crash() {
                // Verify that passing ICC profile does not crash
                let icc = create_minimal_srgb_icc();
                let img = create_test_image(100, 100);
                let result = encode_avif(&img, 60, Some(&icc));
                assert!(result.is_ok(), "AVIF encoding with ICC should succeed");
            }

            #[test]
            fn test_avif_encoding_without_icc() {
                // Verify that encoding works without ICC
                let img = create_test_image(100, 100);
                let avif = encode_avif(&img, 60, None).unwrap();

                // Verify AVIF data is valid
                assert!(is_avif_data(&avif), "Output should be valid AVIF");

                // Should not have ICC profile
                let extracted = extract_icc_ok(&avif);
                assert!(
                    extracted.is_none(),
                    "AVIF without ICC should not have ICC profile"
                );
            }
        }
    }

    mod load_file_safe_tests {
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

    mod exif_raw_tests {
        use super::*;

        #[test]
        fn extract_exif_raw_returns_none_for_png() {
            let png = create_minimal_png();
            assert!(
                extract_exif_raw(&png).is_none(),
                "extract_exif_raw should return None for PNG data"
            );
        }

        #[test]
        fn extract_exif_raw_returns_none_for_webp() {
            let webp = create_minimal_webp();
            assert!(
                extract_exif_raw(&webp).is_none(),
                "extract_exif_raw should return None for WebP data"
            );
        }

        #[test]
        fn extract_exif_raw_returns_none_for_jpeg_without_exif() {
            let jpeg = create_minimal_jpeg();
            assert!(
                extract_exif_raw(&jpeg).is_none(),
                "extract_exif_raw should return None for JPEG without EXIF"
            );
        }

        #[test]
        fn extract_exif_raw_returns_data_for_jpeg_with_exif() {
            let jpeg = create_minimal_jpeg();
            // Inject an EXIF APP1 segment after SOI
            let exif_header = b"Exif\0\0";
            let payload = [0xAA; 20];
            let seg_len = (2 + exif_header.len() + payload.len()) as u16;
            let mut modified = Vec::new();
            modified.extend_from_slice(&jpeg[..2]); // SOI
            modified.push(0xFF);
            modified.push(0xE1); // APP1
            modified.extend_from_slice(&seg_len.to_be_bytes());
            modified.extend_from_slice(exif_header);
            modified.extend_from_slice(&payload);
            modified.extend_from_slice(&jpeg[2..]);

            let result = extract_exif_raw(&modified);
            assert!(
                result.is_some(),
                "extract_exif_raw should return EXIF data for JPEG with EXIF"
            );
        }
    }

    mod exif_presence_tests {
        use super::*;

        /// Build a JPEG containing an EXIF APP1 segment whose payload is a
        /// valid minimal little-endian TIFF structure. `with_gps` toggles
        /// presence of a GPS IFD pointer tag in IFD0.
        fn jpeg_with_exif(with_gps: bool) -> Vec<u8> {
            let base = create_minimal_jpeg();

            // TIFF header (LE) + IFD0 with 1 or 2 entries.
            let mut tiff = Vec::new();
            tiff.extend_from_slice(b"II"); // little-endian
            tiff.extend_from_slice(&0x002A_u16.to_le_bytes()); // magic
            tiff.extend_from_slice(&8u32.to_le_bytes()); // IFD0 offset

            // IFD0: number of entries
            let entry_count: u16 = if with_gps { 2 } else { 1 };
            tiff.extend_from_slice(&entry_count.to_le_bytes());

            // Entry 1: Orientation (0x0112), type=SHORT(3), count=1, value=1
            tiff.extend_from_slice(&0x0112_u16.to_le_bytes());
            tiff.extend_from_slice(&3u16.to_le_bytes());
            tiff.extend_from_slice(&1u32.to_le_bytes());
            tiff.extend_from_slice(&[1u8, 0, 0, 0]);

            if with_gps {
                // Entry 2: GPS IFD pointer (0x8825), type=LONG(4), count=1
                tiff.extend_from_slice(&0x8825_u16.to_le_bytes());
                tiff.extend_from_slice(&4u16.to_le_bytes());
                tiff.extend_from_slice(&1u32.to_le_bytes());
                // Offset points past IFD0; we don't actually parse the GPS IFD.
                tiff.extend_from_slice(&64u32.to_le_bytes());
            }

            // Next-IFD pointer (zero — terminator)
            tiff.extend_from_slice(&0u32.to_le_bytes());

            // Wrap the TIFF in a JPEG APP1 "Exif\0\0" segment.
            let exif_header = b"Exif\0\0";
            let seg_len = (2 + exif_header.len() + tiff.len()) as u16;
            let mut out = Vec::with_capacity(base.len() + tiff.len() + 16);
            out.extend_from_slice(&base[..2]); // SOI
            out.push(0xFF);
            out.push(0xE1); // APP1 marker
            out.extend_from_slice(&seg_len.to_be_bytes());
            out.extend_from_slice(exif_header);
            out.extend_from_slice(&tiff);
            out.extend_from_slice(&base[2..]);
            out
        }

        #[test]
        fn has_exif_returns_false_for_png() {
            assert!(!has_exif(&create_minimal_png()));
        }

        #[test]
        fn has_exif_returns_false_for_jpeg_without_exif() {
            assert!(!has_exif(&create_minimal_jpeg()));
        }

        #[test]
        fn has_exif_returns_true_for_jpeg_with_exif() {
            let jpeg = jpeg_with_exif(false);
            assert!(has_exif(&jpeg));
        }

        /// Regression: `extract_exif_raw` is gated by `MAX_EXIF_SOURCE_BYTES`
        /// (8 MiB), but `has_exif` must NOT be — otherwise metrics report
        /// `metadataStripped: false` for large EXIF-bearing JPEGs.
        #[test]
        fn has_exif_works_beyond_max_exif_source_bytes() {
            let mut jpeg = jpeg_with_exif(false);
            // Pad the source past the 8 MiB cap by inserting a benign APP segment.
            // Insert before SOS so the marker walker still reaches the EXIF segment.
            // We splice a large APP15 (FF EF) segment between EXIF APP1 and the rest.
            let pad_payload_len = MAX_EXIF_SOURCE_BYTES; // pushes total well over 8 MiB
            let seg_len: u16 = (pad_payload_len + 2).min(u16::MAX as usize) as u16;
            // Find the position right after the EXIF APP1 segment we injected.
            // jpeg_with_exif put SOI, then APP1, then the rest. Locate end of APP1.
            // APP1 starts at index 2: FF E1 LL LL <payload of seg_len-2>
            let app1_payload_len = u16::from_be_bytes([jpeg[4], jpeg[5]]) as usize;
            // marker(2) + length(2) + payload
            let app1_end = 4 + app1_payload_len;
            // Inject FF EF <len> <pad bytes> just after APP1
            let mut padded = Vec::with_capacity(jpeg.len() + pad_payload_len + 4);
            padded.extend_from_slice(&jpeg[..app1_end]);
            // Append several large APP15 segments until we exceed the cap
            let single = {
                let mut s = Vec::with_capacity(seg_len as usize + 2);
                s.push(0xFF);
                s.push(0xEF); // APP15
                s.extend_from_slice(&seg_len.to_be_bytes());
                s.resize(s.len() + (seg_len as usize - 2), 0u8);
                s
            };
            // Repeat enough times to cross 8 MiB
            let need = MAX_EXIF_SOURCE_BYTES + single.len();
            let repeats = need / single.len() + 1;
            for _ in 0..repeats {
                padded.extend_from_slice(&single);
            }
            padded.extend_from_slice(&jpeg[app1_end..]);
            jpeg = padded;

            assert!(
                jpeg.len() > MAX_EXIF_SOURCE_BYTES,
                "test buffer must exceed MAX_EXIF_SOURCE_BYTES (got {} bytes)",
                jpeg.len()
            );

            // `extract_exif_raw` bails out due to the cap.
            assert!(
                extract_exif_raw(&jpeg).is_none(),
                "extract_exif_raw is intentionally gated by MAX_EXIF_SOURCE_BYTES"
            );
            // `has_exif` must still find the EXIF marker.
            assert!(
                has_exif(&jpeg),
                "has_exif must detect EXIF in JPEGs larger than MAX_EXIF_SOURCE_BYTES"
            );
        }

        #[test]
        fn exif_has_gps_returns_false_for_empty() {
            assert!(!exif_has_gps(&[]));
            assert!(!exif_has_gps(&[0u8; 4]));
        }

        #[test]
        fn exif_has_gps_returns_false_when_gps_absent() {
            // Pull the embedded EXIF payload back out so we test the TIFF parser
            // on real-shaped bytes.
            let jpeg = jpeg_with_exif(false);
            let payload = find_exif_segment_jpeg_for_tests(&jpeg).expect("EXIF must be present");
            assert!(!exif_has_gps(payload));
        }

        #[test]
        fn exif_has_gps_returns_true_when_gps_present() {
            let jpeg = jpeg_with_exif(true);
            let payload = find_exif_segment_jpeg_for_tests(&jpeg).expect("EXIF must be present");
            assert!(exif_has_gps(payload));
        }

        #[test]
        fn exif_has_gps_handles_big_endian() {
            // Construct a minimal big-endian TIFF with a GPS pointer.
            let mut tiff = Vec::new();
            tiff.extend_from_slice(b"MM");
            tiff.extend_from_slice(&0x002A_u16.to_be_bytes());
            tiff.extend_from_slice(&8u32.to_be_bytes()); // IFD0 offset
            tiff.extend_from_slice(&1u16.to_be_bytes()); // one entry
            tiff.extend_from_slice(&0x8825_u16.to_be_bytes()); // GPS tag
            tiff.extend_from_slice(&4u16.to_be_bytes()); // LONG
            tiff.extend_from_slice(&1u32.to_be_bytes()); // count
            tiff.extend_from_slice(&64u32.to_be_bytes()); // value/offset
            tiff.extend_from_slice(&0u32.to_be_bytes()); // next IFD
            assert!(exif_has_gps(&tiff));
        }

        #[test]
        fn exif_has_gps_rejects_unknown_byte_order() {
            let mut tiff = vec![b'X', b'X']; // not II or MM
            tiff.extend_from_slice(&[0u8; 16]);
            assert!(!exif_has_gps(&tiff));
        }
    }
}
