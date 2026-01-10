// src/engine/io.rs
//
// I/O operations: Source enum, file loading, and ICC profile extraction

use crate::error::LazyImageError;
use img_parts::{jpeg::Jpeg, png::Png, ImageICC};
use libavif_sys::*;
use std::path::PathBuf;
use std::sync::Arc;

/// Image source - supports both in-memory data and file paths (lazy loading)
#[derive(Clone, Debug)]
pub enum Source {
    /// In-memory image data (from Buffer)
    Memory(Arc<Vec<u8>>),
    /// File path for lazy loading (data is read only when needed)
    Path(PathBuf),
}

impl Source {
    /// Load the actual bytes from the source
    pub fn load(&self) -> std::result::Result<Arc<Vec<u8>>, LazyImageError> {
        match self {
            Source::Memory(data) => Ok(data.clone()),
            Source::Path(path) => {
                let data = std::fs::read(path).map_err(|e| {
                    LazyImageError::file_read_failed(path.to_string_lossy().to_string(), e)
                })?;
                Ok(Arc::new(data))
            }
        }
    }

    /// Get path if this is a Path source
    pub fn as_path(&self) -> Option<&PathBuf> {
        match self {
            Source::Path(p) => Some(p),
            Source::Memory(_) => None,
        }
    }

    /// Get the bytes directly if this is a Memory source
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Source::Memory(data) => Some(data.as_slice()),
            Source::Path(_) => None,
        }
    }

    /// Get the length of the source data
    pub fn len(&self) -> usize {
        match self {
            Source::Memory(data) => data.len(),
            Source::Path(_) => 0, // Unknown until loaded
        }
    }
}

/// Extract ICC profile from image data.
/// Supports JPEG (APP2 marker), PNG (iCCP chunk), and WebP (ICCP chunk).
pub fn extract_icc_profile(data: &[u8]) -> Option<Vec<u8>> {
    // Check magic bytes to determine format
    if data.len() < 12 {
        return None;
    }

    let icc_data = if data[0] == 0xFF && data[1] == 0xD8 {
        // JPEG: starts with 0xFF 0xD8
        extract_icc_from_jpeg(data)?
    } else if data[0] == 0x89 && data[1] == 0x50 && data[2] == 0x4E && data[3] == 0x47 {
        // PNG: starts with 0x89 0x50 0x4E 0x47
        extract_icc_from_png(data)?
    } else if &data[0..4] == b"RIFF" && data.len() >= 12 && &data[8..12] == b"WEBP" {
        // WebP: starts with "RIFF" then 4 bytes size then "WEBP"
        extract_icc_from_webp(data)?
    } else if is_avif_data(data) {
        // AVIF: ISOBMFF-based format with 'ftyp' box containing 'avif' brand
        extract_icc_from_avif(data)?
    } else {
        return None;
    };

    // Validate extracted ICC profile
    if validate_icc_profile(&icc_data) {
        Some(icc_data)
    } else {
        // Invalid ICC profile - skip it
        None
    }
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

/// Extract ICC profile from JPEG data
pub(crate) fn extract_icc_from_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    let jpeg = Jpeg::from_bytes(data.to_vec().into()).ok()?;
    jpeg.icc_profile().map(|icc| icc.to_vec())
}

/// Extract ICC profile from PNG data
pub(crate) fn extract_icc_from_png(data: &[u8]) -> Option<Vec<u8>> {
    let png = Png::from_bytes(data.to_vec().into()).ok()?;
    png.icc_profile().map(|icc| icc.to_vec())
}

/// Extract ICC profile from WebP data
pub(crate) fn extract_icc_from_webp(data: &[u8]) -> Option<Vec<u8>> {
    use img_parts::webp::WebP;
    let webp = WebP::from_bytes(data.to_vec().into()).ok()?;
    webp.icc_profile().map(|icc| icc.to_vec())
}

/// Extract ICC profile from AVIF data using libavif
/// libavif-sys is always available (not dependent on napi feature)
fn extract_icc_from_avif(data: &[u8]) -> Option<Vec<u8>> {
    unsafe {
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

        let icc_data = std::slice::from_raw_parts(icc_ptr, icc_size).to_vec();
        Some(icc_data)
    }
}
