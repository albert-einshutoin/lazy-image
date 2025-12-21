// lib.rs
//
// lazy-image: A next-generation image processing engine for Node.js
//
// Design goals:
// - Faster than sharp
// - Smaller output than sharp
// - Better quality than sharp
// - Lazy pipeline execution
// - Non-blocking async API

#[cfg(feature = "napi")]
#[macro_use]
extern crate napi_derive;

pub mod engine;
pub mod error;
pub mod ops;

#[cfg(feature = "napi")]
use image::io::Reader as ImageReader;
#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;
#[cfg(feature = "napi")]
use std::io::Cursor;

// Re-export the engine for NAPI
#[cfg(feature = "napi")]
pub use engine::ImageEngine;
pub use error::ErrorCode;
use error::LazyImageError;

#[cfg(feature = "napi")]
/// Image metadata returned by inspect()
#[napi(object)]
pub struct ImageMetadata {
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// Detected format (jpeg, png, webp, gif, etc.)
    pub format: Option<String>,
}

#[cfg(feature = "napi")]
/// Inspect image metadata WITHOUT decoding pixels.
/// This reads only the header bytes - extremely fast (<1ms).
///
/// Use this to check dimensions before processing, or to reject
/// images that are too large without wasting CPU on decoding.
#[napi]
pub fn inspect(buffer: Buffer) -> Result<ImageMetadata> {
    let cursor = Cursor::new(buffer.as_ref());

    let reader = ImageReader::new(cursor)
        .with_guessed_format()
        .map_err(|e| {
            napi::Error::from(LazyImageError::decode_failed(format!(
                "failed to read image header: {e}"
            )))
        })?;

    // Get format from header (no decoding)
    let format = reader.format().map(|f| format!("{:?}", f).to_lowercase());

    // Get dimensions from header (minimal decoding - just reads header bytes)
    let (width, height) = reader.into_dimensions().map_err(|e| {
        napi::Error::from(LazyImageError::decode_failed(format!(
            "failed to read dimensions: {e}"
        )))
    })?;

    Ok(ImageMetadata {
        width,
        height,
        format,
    })
}

#[cfg(feature = "napi")]
/// Inspect image metadata from a file path WITHOUT loading into Node.js heap.
/// **Memory-efficient**: Reads directly from filesystem, bypassing V8 entirely.
/// This is the recommended way for server-side metadata inspection.
#[napi(js_name = "inspectFile")]
pub fn inspect_file(path: String) -> Result<ImageMetadata> {
    use std::fs::File;
    use std::io::BufReader;

    let file = File::open(&path)
        .map_err(|e| napi::Error::from(LazyImageError::file_read_failed(&path, e)))?;

    let reader = ImageReader::new(BufReader::new(file))
        .with_guessed_format()
        .map_err(|e| {
            napi::Error::from(LazyImageError::decode_failed(format!(
                "failed to read image header: {e}"
            )))
        })?;

    // Get format from header (no decoding)
    let format = reader.format().map(|f| format!("{:?}", f).to_lowercase());

    // Get dimensions from header (minimal decoding - just reads header bytes)
    let (width, height) = reader.into_dimensions().map_err(|e| {
        napi::Error::from(LazyImageError::decode_failed(format!(
            "failed to read dimensions: {e}"
        )))
    })?;

    Ok(ImageMetadata {
        width,
        height,
        format,
    })
}

#[cfg(feature = "napi")]
/// Get library version
#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(feature = "napi")]
/// Get supported input formats
#[napi]
pub fn supported_input_formats() -> Vec<String> {
    vec![
        "jpeg".to_string(),
        "jpg".to_string(),
        "png".to_string(),
        "webp".to_string(),
    ]
}

#[cfg(feature = "napi")]
/// Get supported output formats
#[napi]
pub fn supported_output_formats() -> Vec<String> {
    vec![
        "jpeg".to_string(),
        "jpg".to_string(),
        "png".to_string(),
        "webp".to_string(),
        "avif".to_string(),
    ]
}

/// Processing metrics for performance monitoring
#[cfg(feature = "napi")]
#[napi(object)]
#[derive(Default)]
pub struct ProcessingMetrics {
    /// Time taken to decode the image (milliseconds)
    pub decode_time: f64,
    /// Time taken to apply all operations (milliseconds)
    pub process_time: f64,
    /// Time taken to encode the image (milliseconds)
    pub encode_time: f64,
    /// Peak memory usage during processing (bytes, as u32 for NAPI compatibility)
    pub memory_peak: u32,
}

#[cfg(not(feature = "napi"))]
#[derive(Default)]
pub struct ProcessingMetrics {
    /// Time taken to decode the image (milliseconds)
    pub decode_time: f64,
    /// Time taken to apply all operations (milliseconds)
    pub process_time: f64,
    /// Time taken to encode the image (milliseconds)
    pub encode_time: f64,
    /// Peak memory usage during processing (bytes, as u32 for NAPI compatibility)
    pub memory_peak: u32,
}

#[cfg(feature = "napi")]
#[napi(object)]
pub struct OutputWithMetrics {
    pub data: napi::JsBuffer,
    pub metrics: ProcessingMetrics,
}
