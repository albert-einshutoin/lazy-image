// src/engine/io.rs
//
// I/O operations: Source enum, file loading, and ICC profile extraction

use crate::error::LazyImageError;
use libavif_sys::*;
use memmap2::Mmap;
use std::sync::Arc;

const MAX_ICC_SOURCE_BYTES: usize = 8 * 1024 * 1024; // Hard cap to keep fuzz inputs bounded without breaking large images

pub type IccExtractionResult = std::result::Result<Option<Vec<u8>>, LazyImageError>;

fn icc_decode_error(format: &str, reason: &str) -> LazyImageError {
    LazyImageError::decode_failed(format!("{} ICC extraction failed: {}", format, reason))
}

fn icc_internal_panic(format: &str, reason: &str) -> LazyImageError {
    LazyImageError::internal_panic(format!("{} ICC extraction panic: {}", format, reason))
}

/// Image source - supports both in-memory data and memory-mapped files
#[derive(Clone, Debug)]
pub enum Source {
    /// In-memory image data (from Buffer)
    Memory(Arc<Vec<u8>>),
    /// Memory-mapped file (zero-copy access)
    Mapped(Arc<Mmap>),
}

impl Source {
    /// Load the actual bytes from the source
    /// Note: For Mapped sources, this converts to `Vec<u8>` (defeats zero-copy).
    /// Prefer using as_bytes() for zero-copy access when possible.
    #[deprecated(
        note = "Use as_bytes() for zero-copy access. This method defeats zero-copy by converting Mapped to Vec<u8>."
    )]
    pub fn load(&self) -> std::result::Result<Arc<Vec<u8>>, LazyImageError> {
        match self {
            Source::Memory(data) => Ok(data.clone()),
            Source::Mapped(mmap) => {
                // WARNING: This defeats zero-copy by converting to Vec<u8>
                // For zero-copy access, use as_bytes() instead
                Ok(Arc::new(mmap.as_ref().to_vec()))
            }
        }
    }

    /// Get the bytes directly - zero-copy for both Memory and Mapped sources
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Source::Memory(data) => Some(data.as_slice()),
            Source::Mapped(mmap) => Some(mmap.as_ref()),
        }
    }

    /// Get the length of the source data
    pub fn len(&self) -> usize {
        match self {
            Source::Memory(data) => data.len(),
            Source::Mapped(mmap) => mmap.len(),
        }
    }
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

    let icc_data = if data.starts_with(&[0xFF, 0xD8]) {
        guard_icc_extraction("jpeg", || Ok(extract_icc_from_jpeg(data)))?
    } else if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        guard_icc_extraction("png", || Ok(extract_icc_from_png(data)))?
    } else if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WEBP" {
        guard_icc_extraction("webp", || Ok(extract_icc_from_webp(data)))?
    } else if is_avif_data(data) {
        guard_icc_extraction("avif", || Ok(extract_icc_from_avif_safe(data)))?
    } else {
        return Ok(None);
    };

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
pub(crate) fn extract_icc_from_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    if !is_well_formed_jpeg(data) {
        return None;
    }
    extract_icc_from_jpeg_app2(data)
}

/// Minimal JPEG structure check to avoid handing obviously malformed buffers to parsers.
fn is_well_formed_jpeg(data: &[u8]) -> bool {
    const SOI: u8 = 0xD8;
    const EOI: u8 = 0xD9;
    const SOS: u8 = 0xDA;

    if data.len() < 4 || data[0] != 0xFF || data[1] != SOI {
        return false;
    }

    let mut i = 2; // after SOI
    while i + 1 < data.len() {
        if data[i] != 0xFF {
            return false;
        }
        while i < data.len() && data[i] == 0xFF {
            i += 1;
        }
        if i >= data.len() {
            return false;
        }
        let marker = data[i];
        i += 1;

        // Standalone markers without length
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 || marker == EOI {
            if marker == EOI {
                return true;
            }
            continue;
        }

        if i + 1 >= data.len() {
            return false;
        }
        let seg_len = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
        if seg_len < 2 {
            return false;
        }
        if i + seg_len > data.len() {
            return false;
        }
        i += seg_len;

        if marker == SOS {
            // Require at least one byte of scan data
            return i < data.len();
        }
    }
    false
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

/// Extract ICC profile from PNG data using a deterministic parser (iCCP chunk).
pub(crate) fn extract_icc_from_png(data: &[u8]) -> Option<Vec<u8>> {
    extract_icc_from_png_direct(data)
}

/// Extract ICC profile from PNG iCCP chunk by directly parsing PNG structure
/// This provides a deterministic fallback when img-parts cannot extract the profile.
pub(crate) fn extract_icc_from_png_direct(data: &[u8]) -> Option<Vec<u8>> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    // PNG signature: 0x89 0x50 0x4E 0x47 0x0D 0x0A 0x1A 0x0A
    if data.len() < 8 || &data[0..8] != [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
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

/// Extract ICC profile from WebP data by walking RIFF chunks and reading ICCP.
pub(crate) fn extract_icc_from_webp(data: &[u8]) -> Option<Vec<u8>> {
    extract_icc_from_webp_riff(data)
}

/// Extract ICC profile from WebP RIFF container without invoking img-parts.
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
        let padded = if size % 2 == 0 { size } else { size + 1 };
        offset = offset.saturating_add(padded);
    }

    None
}

/// Extract ICC profile from AVIF data using libavif with panic and size guards.
/// libavif-sys is always available (not dependent on napi feature)
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

// =============================================================================
// EXIF METADATA EXTRACTION AND SANITIZATION
// =============================================================================
//
// Uses little_exif for tag-level EXIF access, enabling:
// - Orientation tag reset after auto-orient (prevents double-rotation bugs)
// - GPS tag stripping by default (privacy-first, exceeds Sharp's capabilities)

use little_exif::filetype::FileExtension;
use little_exif::metadata::Metadata as ExifMetadata;

/// Maximum EXIF data size to process (prevent DoS from malicious inputs)
const MAX_EXIF_SOURCE_BYTES: usize = 8 * 1024 * 1024;

/// Detect file extension for little_exif from image data magic bytes
/// Note: Reserved for future PNG/WebP/AVIF EXIF support
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
pub fn extract_exif_raw(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 12 || data.len() > MAX_EXIF_SOURCE_BYTES {
        return None;
    }

    // For JPEG, extract raw APP1 segment containing EXIF
    if data.starts_with(&[0xFF, 0xD8]) {
        return extract_exif_raw_jpeg(data);
    }

    // For other formats, we'll extract EXIF using little_exif and serialize
    // This is a fallback that may not preserve all metadata perfectly
    let file_ext = detect_file_extension(data)?;
    let data_vec = data.to_vec();

    std::panic::catch_unwind(|| {
        let _metadata = ExifMetadata::new_from_vec(&data_vec, file_ext).ok()?;
        // Serialize the metadata to raw bytes
        // little_exif doesn't have direct serialization, so we work with what we have
        // For non-JPEG, we'll store a marker and rely on little_exif at write time
        Some(data_vec) // Store original for now, sanitize at write time
    })
    .ok()
    .flatten()
}

/// Extract raw EXIF APP1 segment from JPEG data
fn extract_exif_raw_jpeg(data: &[u8]) -> Option<Vec<u8>> {
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
                // Return the entire EXIF data (without the EXIF identifier)
                return Some(segment[6..].to_vec());
            }
        }

        i = seg_end;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::encoder::{encode_avif, encode_jpeg, encode_png, encode_webp};
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
            let result = extract_icc_from_jpeg(&jpeg_data);
            assert!(result.is_none());
        }

        #[test]
        fn test_extract_icc_from_png_no_profile() {
            // PNG without ICC profile
            let png_data = create_minimal_png();
            let result = extract_icc_from_png(&png_data);
            assert!(result.is_none());
        }

        #[test]
        fn test_extract_icc_from_webp_no_profile() {
            // WebP without ICC profile
            let webp_data = create_minimal_webp();
            let result = extract_icc_from_webp(&webp_data);
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
}
