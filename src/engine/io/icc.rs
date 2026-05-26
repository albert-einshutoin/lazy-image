// src/engine/io/icc.rs
//
// ICC profile extraction across JPEG, PNG, WebP, and AVIF containers.
//
// The `IccExtractor` trait dispatches format-specific parsers while
// `extract_and_validate_icc` owns the cross-cutting concerns (panic guard +
// ICC header validation). `is_avif_data` lives here as well because its only
// production consumer is AVIF ICC extractor selection.

use crate::error::LazyImageError;
#[cfg(feature = "avif")]
use libavif_sys::*;

/// Hard cap on input size to keep fuzz inputs bounded without breaking large
/// images. Inputs larger than this skip ICC parsing entirely.
const MAX_ICC_SOURCE_BYTES: usize = 8 * 1024 * 1024;

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
        // SAFETY:
        // - `decoder` is created by `avifDecoderCreate` and released by `AvifDecoderGuard`.
        // - `avifDecoderSetIOMemory` borrows `data` for the duration of this call only, and
        //   `data` lives for the entire function.
        // - After `avifDecoderParse` returns `AVIF_RESULT_OK`, libavif guarantees that
        //   `decoder.image` and its ICC fields point to initialized memory owned by the decoder
        //   until the decoder is destroyed.
        // Create decoder
        let decoder = avifDecoderCreate();
        if decoder.is_null() {
            return None;
        }

        // Set up RAII cleanup
        struct AvifDecoderGuard(*mut avifDecoder);
        impl Drop for AvifDecoderGuard {
            fn drop(&mut self) {
                // SAFETY: `self.0` was created by `avifDecoderCreate` on the path that
                // constructed this guard, so `avifDecoderDestroy` is the matching destructor
                // and is safe to call after the null check below. The pointer is uniquely
                // owned by the guard, so there is no aliasing concern.
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

#[cfg(test)]
#[path = "icc_tests.rs"]
mod tests;
