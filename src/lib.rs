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

    let file =
        File::open(path).map_err(|e| LazyImageError::file_read_failed(path.to_string(), e))?;
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
pub fn inspect(env: Env, buffer: Buffer) -> Result<ImageMetadata> {
    let metadata = match inspect_header_from_bytes(buffer.as_ref()) {
        Ok(metadata) => metadata,
        Err(err) => {
            return Err(crate::error::napi_error_with_code(&env, err.clone())?);
        }
    };
    Ok(metadata.into())
}

#[cfg(feature = "napi")]
/// Inspect image metadata from a file path WITHOUT loading into Node.js heap.
/// **Memory-efficient**: Reads directly from filesystem, bypassing V8 entirely.
/// This is the recommended way for server-side metadata inspection.
#[napi(js_name = "inspectFile")]
pub fn inspect_file(env: Env, path: String) -> Result<ImageMetadata> {
    let metadata = match inspect_header_from_path(&path) {
        Ok(metadata) => metadata,
        Err(err) => {
            return Err(crate::error::napi_error_with_code(&env, err.clone())?);
        }
    };
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

/// Metrics payload version. Keep in sync with docs/metrics-schema.json
pub const PROCESSING_METRICS_VERSION: &str = "1.0.0";

/// Processing metrics for performance monitoring
#[cfg(feature = "napi")]
#[napi(object)]
pub struct ProcessingMetrics {
    /// Schema version for compatibility negotiation
    pub version: String,
    /// Decode stage duration in milliseconds
    pub decode_ms: f64,
    /// Ops (transform) stage duration in milliseconds
    pub ops_ms: f64,
    /// Encode stage duration in milliseconds
    pub encode_ms: f64,
    /// Total wall-clock duration in milliseconds
    pub total_ms: f64,
    /// Peak memory usage during processing (RSS, bytes, as u32 for NAPI compatibility)
    ///
    /// **Note**: On Linux/macOS, this uses `ru_maxrss` from `getrusage()`, which represents
    /// the cumulative maximum RSS of the entire process, not just this operation.
    /// This is a limitation of the `getrusage()` API. For accurate per-operation memory tracking,
    /// consider using process-specific memory profiling tools.
    pub peak_rss: u32,
    /// Total CPU time (user + system) in seconds
    pub cpu_time: f64,
    /// Total processing time (wall clock) in seconds (legacy seconds field)
    pub processing_time: f64,
    /// Input file size in bytes (as u32 for NAPI compatibility, max 4GB)
    pub bytes_in: u32,
    /// Output file size in bytes (as u32 for NAPI compatibility, max 4GB)
    pub bytes_out: u32,
    /// Compression ratio (bytes_out / bytes_in)
    pub compression_ratio: f64,
    /// Detected input format (lowercase: jpeg, png, webp, avif, etc.)
    pub format_in: Option<String>,
    /// Output format
    pub format_out: String,
    /// True when ICC profile was present and preserved
    pub icc_preserved: bool,
    /// True when metadata was stripped (either by default or policy)
    pub metadata_stripped: bool,
    /// Non-fatal policy rejections (e.g., strict policy forcing metadata strip)
    pub policy_violations: Vec<String>,
    // ----------------------------------------------------------------------
    // Legacy fields preserved for backward compatibility
    /// Time taken to decode the image (milliseconds) - legacy alias of decode_ms
    pub decode_time: f64,
    /// Time taken to apply all operations (milliseconds) - legacy alias of ops_ms
    pub process_time: f64,
    /// Time taken to encode the image (milliseconds) - legacy alias of encode_ms
    pub encode_time: f64,
    /// Peak memory usage during processing (RSS, bytes) - legacy alias of peak_rss
    pub memory_peak: u32,
    /// Input size legacy alias (bytes_in)
    pub input_size: u32,
    /// Output size legacy alias (bytes_out)
    pub output_size: u32,
}

#[cfg(not(feature = "napi"))]
pub struct ProcessingMetrics {
    /// Schema version for compatibility negotiation
    pub version: String,
    /// Decode stage duration in milliseconds
    pub decode_ms: f64,
    /// Ops (transform) stage duration in milliseconds
    pub ops_ms: f64,
    /// Encode stage duration in milliseconds
    pub encode_ms: f64,
    /// Total wall-clock duration in milliseconds
    pub total_ms: f64,
    /// Peak memory usage during processing (RSS, bytes, as u32 for NAPI compatibility)
    ///
    /// **Note**: On Linux/macOS, this uses `ru_maxrss` from `getrusage()`, which represents
    /// the cumulative maximum RSS of the entire process, not just this operation.
    /// This is a limitation of the `getrusage()` API. For accurate per-operation memory tracking,
    /// consider using process-specific memory profiling tools.
    pub peak_rss: u32,
    /// Total CPU time (user + system) in seconds
    pub cpu_time: f64,
    /// Total processing time (wall clock) in seconds (legacy seconds field)
    pub processing_time: f64,
    /// Input file size in bytes (as u32 for NAPI compatibility, max 4GB)
    pub bytes_in: u32,
    /// Output file size in bytes (as u32 for NAPI compatibility, max 4GB)
    pub bytes_out: u32,
    /// Compression ratio (bytes_out / bytes_in)
    pub compression_ratio: f64,
    /// Detected input format (lowercase: jpeg, png, webp, avif, etc.)
    pub format_in: Option<String>,
    /// Output format
    pub format_out: String,
    /// True when ICC profile was present and preserved
    pub icc_preserved: bool,
    /// True when metadata was stripped (either by default or policy)
    pub metadata_stripped: bool,
    /// Non-fatal policy rejections (e.g., strict policy forcing metadata strip)
    pub policy_violations: Vec<String>,
    // ----------------------------------------------------------------------
    // Legacy fields preserved for backward compatibility
    /// Time taken to decode the image (milliseconds) - legacy alias of decode_ms
    pub decode_time: f64,
    /// Time taken to apply all operations (milliseconds) - legacy alias of ops_ms
    pub process_time: f64,
    /// Time taken to encode the image (milliseconds) - legacy alias of encode_ms
    pub encode_time: f64,
    /// Peak memory usage during processing (RSS, bytes) - legacy alias of peak_rss
    pub memory_peak: u32,
    /// Input size legacy alias (bytes_in)
    pub input_size: u32,
    /// Output size legacy alias (bytes_out)
    pub output_size: u32,
}

#[cfg(feature = "napi")]
impl Default for ProcessingMetrics {
    fn default() -> Self {
        Self {
            version: PROCESSING_METRICS_VERSION.to_string(),
            decode_ms: 0.0,
            ops_ms: 0.0,
            encode_ms: 0.0,
            total_ms: 0.0,
            peak_rss: 0,
            cpu_time: 0.0,
            processing_time: 0.0,
            bytes_in: 0,
            bytes_out: 0,
            compression_ratio: 0.0,
            format_in: None,
            format_out: String::new(),
            icc_preserved: false,
            metadata_stripped: true,
            policy_violations: Vec::new(),
            decode_time: 0.0,
            process_time: 0.0,
            encode_time: 0.0,
            memory_peak: 0,
            input_size: 0,
            output_size: 0,
        }
    }
}

#[cfg(not(feature = "napi"))]
impl Default for ProcessingMetrics {
    fn default() -> Self {
        Self {
            version: PROCESSING_METRICS_VERSION.to_string(),
            decode_ms: 0.0,
            ops_ms: 0.0,
            encode_ms: 0.0,
            total_ms: 0.0,
            peak_rss: 0,
            cpu_time: 0.0,
            processing_time: 0.0,
            bytes_in: 0,
            bytes_out: 0,
            compression_ratio: 0.0,
            format_in: None,
            format_out: String::new(),
            icc_preserved: false,
            metadata_stripped: true,
            policy_violations: Vec::new(),
            decode_time: 0.0,
            process_time: 0.0,
            encode_time: 0.0,
            memory_peak: 0,
            input_size: 0,
            output_size: 0,
        }
    }
}

#[cfg(feature = "napi")]
#[napi(object)]
pub struct OutputWithMetrics {
    pub data: napi::JsBuffer,
    pub metrics: ProcessingMetrics,
}
