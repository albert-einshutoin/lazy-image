// lib.rs
//
// lazy-image: Web image optimization engine for Node.js
//
// Design goals:
// - Smaller web outputs than sharp defaults
// - Predictable memory usage and safer operational behavior
// - Lazy, non-destructive pipeline execution
// - Competitive AVIF support without optimizing for universal throughput wins
// - Non-blocking async API
//
// See docs/PROJECT_PHILOSOPHY.md for the project's priorities and non-goals.

// Require every unsafe operation inside an `unsafe fn` to sit in an explicit
// `unsafe {}` block (so each one carries its own `// SAFETY:` justification),
// crate-wide rather than only in `codecs::avif_safe`.
#![deny(unsafe_op_in_unsafe_fn)]

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

    let format = reader.format().map(|f| format!("{f:?}").to_lowercase());
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
    if path.trim().is_empty() {
        return Err(crate::error::napi_error_with_code(
            &env,
            LazyImageError::invalid_argument("path", "<empty>", "path must not be empty"),
        )?);
    }

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
    #[cfg(feature = "avif")]
    {
        vec![
            "jpeg".to_string(),
            "jpg".to_string(),
            "png".to_string(),
            "webp".to_string(),
            "avif".to_string(),
        ]
    }
    #[cfg(not(feature = "avif"))]
    {
        vec![
            "jpeg".to_string(),
            "jpg".to_string(),
            "png".to_string(),
            "webp".to_string(),
        ]
    }
}

/// Metrics payload version. Keep in sync with docs/metrics-schema.json
pub const PROCESSING_METRICS_VERSION: &str = "1.0.0";

/// Processing metrics for performance monitoring
#[derive(Debug)]
#[cfg_attr(feature = "napi", napi(object))]
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
}

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
        }
    }
}

#[cfg(feature = "napi")]
#[napi(object)]
pub struct OutputWithMetrics {
    pub data: napi::bindgen_prelude::Buffer,
    pub metrics: ProcessingMetrics,
}

/// Options object for the unified `encode()` / `encodeToFile()` methods.
#[cfg(feature = "napi")]
#[napi(object)]
pub struct EncodeOptions {
    /// Output format: "jpeg", "jpg", "png", "webp", "avif"
    pub format: Option<String>,
    /// Quality 1-100 for lossy formats (default: JPEG=85, WebP=80, AVIF=60). Omit for PNG.
    pub quality: Option<f64>,
    /// If true, uses faster JPEG encoding (2-4x faster, slightly larger files). Default: false.
    pub fast_mode: Option<bool>,
    /// Preset name ("thumbnail", "avatar", "hero", "social").
    /// When specified, format/quality/fastMode are ignored and the preset's
    /// recommended settings are used instead.
    pub preset: Option<String>,
    /// If true, include ProcessingMetrics in the result. Default: false.
    pub metrics: Option<bool>,
}

/// Result from `encode()`.
#[cfg(feature = "napi")]
#[napi(object)]
pub struct EncodeResult {
    /// Encoded image data
    pub data: napi::bindgen_prelude::Buffer,
    /// Processing metrics (only present when `metrics: true` was requested)
    pub metrics: Option<ProcessingMetrics>,
}

/// Result from `encodeToFile()`.
#[cfg(feature = "napi")]
#[derive(Debug)]
#[napi(object)]
pub struct FileEncodeResult {
    /// Number of bytes written to disk
    pub bytes_written: u32,
    /// Processing metrics (only present when `metrics: true` was requested)
    pub metrics: Option<ProcessingMetrics>,
}

#[cfg(feature = "napi")]
#[derive(Debug)]
#[napi(object)]
pub struct FileOutputWithMetrics {
    pub bytes_written: u32,
    pub metrics: ProcessingMetrics,
}

#[cfg(feature = "napi")]
#[derive(Debug)]
#[napi(object)]
pub struct BatchResultWithMetrics {
    pub source: String,
    pub success: bool,
    pub error: Option<String>,
    pub output_path: Option<String>,
    pub error_code: Option<String>,
    pub error_category: Option<error::ErrorCategory>,
    pub effective_concurrency: Option<u32>,
    pub auto_concurrency: Option<bool>,
    pub metrics: Option<ProcessingMetrics>,
}

#[cfg(feature = "napi")]
#[derive(Debug)]
#[napi(object)]
pub struct BatchMetricsSummary {
    pub total_items: u32,
    pub successful_items: u32,
    pub failed_items: u32,
    pub effective_concurrency: u32,
    pub auto_concurrency: bool,
    pub total_bytes_in: f64,
    pub total_bytes_out: f64,
    pub total_decode_ms: f64,
    pub total_ops_ms: f64,
    pub total_encode_ms: f64,
    pub total_cpu_time: f64,
    pub total_wall_ms: f64,
}

#[cfg(feature = "napi")]
#[derive(Debug)]
#[napi(object)]
pub struct BatchOutputWithMetrics {
    pub items: Vec<BatchResultWithMetrics>,
    pub summary: BatchMetricsSummary,
}

/// Result of byte-budget encoding (binary search over quality).
///
/// The binary search runs entirely in Rust, eliminating JS↔NAPI round-trips
/// that the previous JS-side implementation required (~7 per call).
#[cfg(feature = "napi")]
#[napi(object)]
pub struct TargetBytesResult {
    /// Encoded image data at the chosen quality
    pub data: napi::bindgen_prelude::Buffer,
    /// Quality level that was selected
    pub quality: u32,
    /// Actual output size in bytes
    pub bytes_out: u32,
    /// Whether the byte budget was met
    pub budget_met: bool,
    /// Requested target bytes
    pub target_bytes: u32,
    /// Processing metrics from the final encode iteration
    pub metrics: ProcessingMetrics,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cargo_pkg_version_is_semver() {
        // version() is NAPI-only, but the underlying env!("CARGO_PKG_VERSION")
        // is always available. Verify it is a valid semver string.
        let version = env!("CARGO_PKG_VERSION");
        assert!(!version.is_empty(), "CARGO_PKG_VERSION must be non-empty");
        semver::Version::parse(version)
            .unwrap_or_else(|_| panic!("version '{version}' should be valid semver"));
    }

    #[test]
    fn test_processing_metrics_version_constant() {
        assert_eq!(PROCESSING_METRICS_VERSION, "1.0.0");
    }

    #[test]
    fn test_processing_metrics_default() {
        let m = ProcessingMetrics::default();
        assert_eq!(m.version, PROCESSING_METRICS_VERSION);
        assert_eq!(m.decode_ms, 0.0);
        assert_eq!(m.ops_ms, 0.0);
        assert_eq!(m.encode_ms, 0.0);
        assert_eq!(m.total_ms, 0.0);
        assert_eq!(m.peak_rss, 0);
        assert_eq!(m.bytes_in, 0);
        assert_eq!(m.bytes_out, 0);
        assert_eq!(m.compression_ratio, 0.0);
        assert!(m.format_in.is_none());
        assert!(m.format_out.is_empty());
        assert!(!m.icc_preserved);
        assert!(m.metadata_stripped);
        assert!(m.policy_violations.is_empty());
    }
}
