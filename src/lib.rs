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

// Memory allocator optimization - jemalloc for better performance
// Expected impact: 10-15% overall performance improvement
// Note: jemalloc is not supported on Windows/MSVC, so we exclude it on that platform
#[cfg(all(feature = "jemalloc", not(target_env = "msvc")))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

pub mod codecs;
pub mod engine;
pub mod error;
pub mod ops;

#[cfg(any(feature = "napi", feature = "fuzzing"))]
use image::ImageReader;
#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;
#[cfg(any(feature = "napi", feature = "fuzzing"))]
use std::io::{BufRead, BufReader, Cursor, Seek};

// Re-export the engine for NAPI
#[cfg(feature = "napi")]
pub use engine::ImageEngine;
#[cfg(any(feature = "napi", feature = "fuzzing"))]
use error::LazyImageError;

#[cfg(any(feature = "napi", feature = "fuzzing"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectMetadata {
    pub width: u32,
    pub height: u32,
    pub format: Option<String>,
}

#[cfg(any(feature = "napi", feature = "fuzzing"))]
fn read_inspect_metadata<R: BufRead + Seek>(
    reader: R,
) -> std::result::Result<InspectMetadata, LazyImageError> {
    let reader = ImageReader::new(reader)
        .with_guessed_format()
        .map_err(|e| LazyImageError::decode_failed(format!("failed to read image header: {e}")))?;

    let format = reader.format().map(|f| format!("{:?}", f).to_lowercase());
    let (width, height) = reader
        .into_dimensions()
        .map_err(|e| LazyImageError::decode_failed(format!("failed to read dimensions: {e}")))?;

    Ok(InspectMetadata {
        width,
        height,
        format,
    })
}

#[cfg(any(feature = "napi", feature = "fuzzing"))]
pub fn inspect_header_from_bytes(
    data: &[u8],
) -> std::result::Result<InspectMetadata, LazyImageError> {
    read_inspect_metadata(Cursor::new(data))
}

#[cfg(any(feature = "napi", feature = "fuzzing"))]
pub fn inspect_header_from_path(
    path: &str,
) -> std::result::Result<InspectMetadata, LazyImageError> {
    use std::fs::File;

    let file = File::open(path).map_err(|e| LazyImageError::file_read_failed(path.to_string(), e))?;
    read_inspect_metadata(BufReader::new(file))
}

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
impl From<InspectMetadata> for ImageMetadata {
    fn from(value: InspectMetadata) -> Self {
        Self {
            width: value.width,
            height: value.height,
            format: value.format,
        }
    }
}

#[cfg(feature = "napi")]
/// Inspect image metadata WITHOUT decoding pixels.
/// This reads only the header bytes - extremely fast (<1ms).
///
/// Use this to check dimensions before processing, or to reject
/// images that are too large without wasting CPU on decoding.
#[napi]
pub fn inspect(buffer: Buffer) -> Result<ImageMetadata> {
    let metadata = inspect_header_from_bytes(buffer.as_ref()).map_err(napi::Error::from)?;
    Ok(metadata.into())
}

#[cfg(feature = "napi")]
/// Inspect image metadata from a file path WITHOUT loading into Node.js heap.
/// **Memory-efficient**: Reads directly from filesystem, bypassing V8 entirely.
/// This is the recommended way for server-side metadata inspection.
#[napi(js_name = "inspectFile")]
pub fn inspect_file(path: String) -> Result<ImageMetadata> {
    let metadata = inspect_header_from_path(&path).map_err(napi::Error::from)?;
    Ok(metadata.into())
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
