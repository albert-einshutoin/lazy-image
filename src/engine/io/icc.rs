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
// Bounds decompression before policy-specific checks; profiles above the
// always-on 10 MiB ceiling can never be accepted.
const MAX_EXTRACTED_ICC_BYTES: u64 = 10 * 1024 * 1024 + 4096;

pub type IccExtractionResult = std::result::Result<Option<Vec<u8>>, LazyImageError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IccSourceState {
    Absent,
    Valid,
    Unsafe,
}

pub struct ClassifiedIcc {
    pub state: IccSourceState,
    pub profile: Option<Vec<u8>>,
}

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

fn has_icc_payload(data: &[u8]) -> bool {
    if data.starts_with(&[0xFF, 0xD8]) {
        let mut offset = 2;
        while offset + 1 < data.len() {
            if data[offset] != 0xFF {
                return false;
            }
            while offset < data.len() && data[offset] == 0xFF {
                offset += 1;
            }
            if offset >= data.len() {
                return false;
            }
            let marker = data[offset];
            offset += 1;
            if marker == 0xDA || marker == 0xD9 {
                break;
            }
            if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
                continue;
            }
            if offset + 1 >= data.len() {
                return false;
            }
            let size = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
            let Some(end) = offset.checked_add(size) else {
                return false;
            };
            if size < 2 || end > data.len() {
                return false;
            }
            if marker == 0xE2 && data[offset + 2..end].starts_with(b"ICC_PROFILE\0") {
                return true;
            }
            offset = end;
        }
        return false;
    }

    let (mut offset, kind, is_png) = if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        (8usize, b"iCCP".as_slice(), true)
    } else if data.starts_with(b"RIFF") && data.len() >= 12 && &data[8..12] == b"WEBP" {
        (12usize, b"ICCP".as_slice(), false)
    } else {
        return false;
    };

    while offset + 8 <= data.len() {
        let (chunk_kind, payload_start, padded) = if is_png {
            let size = u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
            (
                &data[offset + 4..offset + 8],
                offset + 8,
                size.saturating_add(4),
            )
        } else {
            let size =
                u32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap()) as usize;
            (
                &data[offset..offset + 4],
                offset + 8,
                size.next_multiple_of(2),
            )
        };
        let Some(end) = payload_start.checked_add(padded) else {
            break;
        };
        if end > data.len() {
            break;
        }
        if chunk_kind == kind {
            return true;
        }
        offset = end;
    }
    false
}

fn classify_icc_profile_inner(data: &[u8], allow_public_source_size: bool) -> ClassifiedIcc {
    if !has_icc_payload(data) {
        return ClassifiedIcc {
            state: IccSourceState::Absent,
            profile: None,
        };
    }
    if (!allow_public_source_size && data.len() > MAX_ICC_SOURCE_BYTES)
        || (allow_public_source_size && data.len() > 32 * 1024 * 1024)
    {
        return ClassifiedIcc {
            state: IccSourceState::Unsafe,
            profile: None,
        };
    }

    let profile = select_icc_extractor(data)
        .and_then(|extractor| extract_and_validate_icc(extractor, data).ok().flatten());
    match profile {
        Some(profile) => ClassifiedIcc {
            state: IccSourceState::Valid,
            profile: Some(profile),
        },
        None => ClassifiedIcc {
            state: IccSourceState::Unsafe,
            profile: None,
        },
    }
}

pub fn classify_icc_profile(data: &[u8]) -> ClassifiedIcc {
    classify_icc_profile_inner(data, false)
}

pub fn classify_public_upload_icc(data: &[u8]) -> ClassifiedIcc {
    classify_icc_profile_inner(data, true)
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
fn is_printable_ascii_field(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .all(|&byte| (32..=126).contains(&byte) || byte == 0)
}

pub(crate) fn validate_icc_profile(icc_data: &[u8]) -> bool {
    const TAG_TABLE_START: usize = 132;
    const MAX_TAGS: usize = 4096;

    // Header plus the required tag-count field, padded to a 4-byte boundary.
    if icc_data.len() < TAG_TABLE_START || !icc_data.len().is_multiple_of(4) {
        return false;
    }

    // Check profile size field (bytes 0-3, big-endian)
    let profile_size =
        u32::from_be_bytes([icc_data[0], icc_data[1], icc_data[2], icc_data[3]]) as usize;

    // Profile size must match actual data length
    if profile_size != icc_data.len() {
        return false;
    }

    if !is_printable_ascii_field(&icc_data[4..8])
        || !matches!(&icc_data[12..16], b"mntr" | b"scnr" | b"prtr")
        || !matches!(&icc_data[16..20], b"RGB " | b"GRAY")
        || !matches!(&icc_data[20..24], b"XYZ " | b"Lab ")
        || &icc_data[36..40] != b"acsp"
        || icc_data[100..128].iter().any(|&byte| byte != 0)
    {
        return false;
    }

    if !matches!(icc_data[8], 2 | 4 | 5) {
        return false;
    }

    let tag_count = u32::from_be_bytes(icc_data[128..132].try_into().unwrap()) as usize;
    if tag_count > MAX_TAGS {
        return false;
    }
    let Some(table_end) = tag_count
        .checked_mul(12)
        .and_then(|bytes| TAG_TABLE_START.checked_add(bytes))
    else {
        return false;
    };
    if table_end > icc_data.len() {
        return false;
    }

    let mut signatures = std::collections::HashSet::with_capacity(tag_count);
    let mut ranges = Vec::with_capacity(tag_count);
    for entry in icc_data[TAG_TABLE_START..table_end].chunks_exact(12) {
        let signature: [u8; 4] = entry[0..4].try_into().unwrap();
        if !signature.iter().all(|byte| (32..=126).contains(byte)) || !signatures.insert(signature)
        {
            return false;
        }

        let offset = u32::from_be_bytes(entry[4..8].try_into().unwrap()) as usize;
        let size = u32::from_be_bytes(entry[8..12].try_into().unwrap()) as usize;
        let Some(end) = offset.checked_add(size) else {
            return false;
        };
        if offset < table_end
            || !offset.is_multiple_of(4)
            || size < 8
            || end > icc_data.len()
            || icc_data[offset + 4..offset + 8]
                .iter()
                .any(|&byte| byte != 0)
        {
            return false;
        }
        ranges.push((offset, end));
    }

    ranges.sort_unstable_by_key(|range| range.0);
    if ranges
        .windows(2)
        .any(|pair| pair[0] != pair[1] && pair[0].1 > pair[1].0)
    {
        return false;
    }

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
            let decoder = ZlibDecoder::new(compressed_data);
            let mut decompressed = Vec::new();
            if decoder
                .take(MAX_EXTRACTED_ICC_BYTES)
                .read_to_end(&mut decompressed)
                .is_ok()
            {
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
        //   The returned pointer is either null (handled immediately) or uniquely owned by
        //   this block; no other code aliases it until `AvifDecoderGuard::drop` runs.
        // - `avifDecoderSetIOMemory` borrows `data` for the duration of this call only, and
        //   `data` lives for the entire function, so the slice pointer and length passed to
        //   libavif remain valid throughout all subsequent libavif calls in this block.
        // - The field write `(*decoder).maxThreads = 1` (line 543) is safe because `decoder`
        //   is a non-null, uniquely-owned live `avifDecoder` allocated by libavif on the
        //   preceding line, and `maxThreads` is a plain integer field with no aliasing
        //   or validity constraint beyond being written before `avifDecoderParse` is called.
        // - After `avifDecoderParse` returns `AVIF_RESULT_OK`, libavif guarantees that
        //   `decoder.image` points to an initialized, libavif-owned `avifImage` and that
        //   `image.icc.data` (if non-null) and `image.icc.size` describe a contiguous,
        //   initialized buffer of exactly `icc.size` bytes owned by the decoder allocation.
        // - `std::slice::from_raw_parts(icc_ptr, icc_size)` is valid because: `icc_ptr` is
        //   checked non-null immediately before the call; `icc_size` was checked against
        //   `MAX_ICC_SOURCE_BYTES` so it is bounded; both pointer and length come from the
        //   libavif-populated `image.icc` struct whose invariants are guaranteed by the
        //   successful `avifDecoderParse` call above; and the resulting slice is immediately
        //   copied via `.to_vec()`, so it does not outlive the decoder or `data`.
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
