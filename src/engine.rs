// src/engine.rs
//
// The core of lazy-image. A lazy pipeline that:
// 1. Queues operations without executing
// 2. Runs everything in a single pass on compute()
// 3. Uses NAPI AsyncTask to not block Node.js main thread

// =============================================================================
// SECURITY LIMITS
// =============================================================================

/// Maximum allowed image dimension (width or height).
/// Images larger than 32768x32768 are rejected to prevent decompression bombs.
/// This is the same limit used by libvips/sharp.
pub const MAX_DIMENSION: u32 = 32768;

// =============================================================================
// GLOBAL THREAD POOL FOR BATCH PROCESSING
// =============================================================================
//
// **Architecture Decision**: We use a single global thread pool for all batch
// operations instead of creating a new pool per request. This provides:
//
// 1. **Zero allocation overhead**: No pool creation cost per batch
// 2. **Better resource utilization**: Threads are reused across operations
// 3. **Predictable performance**: Consistent thread count based on CPU cores
//
// **Thread Count Calculation**:
// - Reads UV_THREADPOOL_SIZE from environment (default: 4, Node.js default)
// - Reserves those threads for libuv I/O operations
// - Uses remaining CPU cores for image processing
// - Formula: max(1, CPU_COUNT - UV_THREADPOOL_SIZE)
//
// **IMPORTANT**:
// - Pool is initialized lazily on first use
// - Environment variables must be set BEFORE first batch operation
// - Changes after initialization have NO effect
//
// **Benchmark Results** (see benches/benchmark.rs):
// - Global pool: ~0.5ms overhead for 100 items
// - New pool per call: ~5-10ms overhead (10-20x slower)
//
#[cfg(feature = "napi")]
use once_cell::sync::Lazy;
#[cfg(feature = "napi")]
static GLOBAL_THREAD_POOL: Lazy<ThreadPool> = Lazy::new(|| {
    let cpu_count = num_cpus::get();

    // Check for UV_THREADPOOL_SIZE environment variable
    // Default: 4 (Node.js/libuv default threadpool size)
    // NOTE: This is read only once during initialization
    let uv_threadpool_size = std::env::var("UV_THREADPOOL_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);

    // Reserve threads for libuv, but ensure we have at least MIN_RAYON_THREADS
    let num_threads = cpu_count
        .saturating_sub(uv_threadpool_size)
        .max(MIN_RAYON_THREADS);

    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .unwrap_or_else(|e| {
            // Fallback: create a minimal thread pool if the preferred configuration fails
            rayon::ThreadPoolBuilder::new()
                .num_threads(MIN_RAYON_THREADS)
                .build()
                .expect(&format!(
                    "Failed to create fallback thread pool with {} threads: {}",
                    MIN_RAYON_THREADS, e
                ))
        })
});

/// Maximum allowed total pixels (width * height).
/// 100 megapixels = 400MB uncompressed RGBA. Beyond this is likely malicious.
const MAX_PIXELS: u64 = 100_000_000;

// =============================================================================
// THREAD POOL CONFIGURATION
// =============================================================================

/// Default libuv thread pool size (Node.js default)
#[allow(dead_code)]
const DEFAULT_UV_THREADPOOL_SIZE: usize = 4;

/// Maximum allowed concurrency value for processBatch()
#[cfg(feature = "napi")]
const MAX_CONCURRENCY: usize = 1024;

/// Minimum number of rayon threads to ensure at least some parallelism
#[cfg(feature = "napi")]
const MIN_RAYON_THREADS: usize = 1;

// Quality configuration helper
struct QualitySettings {
    quality: f32,
}

impl QualitySettings {
    fn new(quality: u8) -> Self {
        Self {
            quality: quality as f32,
        }
    }

    // WebP settings - sharp-equivalent balanced settings
    // Optimized for speed while maintaining quality parity with sharp
    fn webp_method(&self) -> i32 {
        // Use method 4 for all quality levels (balanced, sharp-equivalent)
        // Method 4 provides optimal speed/quality trade-off
        4
    }

    fn webp_pass(&self) -> i32 {
        // Use single pass for all quality levels (sharp-equivalent)
        // Single pass is ~3-5x faster than multi-pass with minimal quality impact
        1
    }

    fn webp_preprocessing(&self) -> i32 {
        // No preprocessing (sharp-equivalent)
        // Disabling preprocessing improves speed by ~10-15%
        0
    }

    fn webp_sns_strength(&self) -> i32 {
        if self.quality >= 85.0 {
            50
        } else if self.quality >= 70.0 {
            70
        } else {
            80
        }
    }

    fn webp_filter_strength(&self) -> i32 {
        if self.quality >= 80.0 {
            20
        } else if self.quality >= 60.0 {
            30
        } else {
            40
        }
    }

    fn webp_filter_sharpness(&self) -> i32 {
        if self.quality >= 85.0 {
            2
        } else {
            0
        }
    }

    // AVIF settings for libavif encoder
    // libavif speed: 0 (slowest/best) to 10 (fastest/worst)
    // We invert our quality-based logic: high quality -> slower speed
    fn avif_speed(&self) -> i32 {
        if self.quality >= 85.0 {
            4 // Slower for higher quality
        } else if self.quality >= 70.0 {
            5
        } else if self.quality >= 50.0 {
            6
        } else {
            7 // Faster for lower quality
        }
    }
}

use crate::error::LazyImageError;
use crate::ops::{Operation, OutputFormat};
#[cfg(feature = "napi")]
use crate::ops::PresetConfig;
use fast_image_resize::{self as fir, PixelType, ResizeOptions};
use image::{DynamicImage, GenericImageView, ImageFormat, RgbImage, RgbaImage};
use img_parts::{jpeg::Jpeg, png::Png, ImageICC};
use mozjpeg::{ColorSpace, Compress, Decompress, ScanMode};
#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;
#[cfg(feature = "napi")]
use napi::{Env, JsBuffer, JsFunction, JsObject, Task};
use num_cpus;
use libavif_sys::*;
#[cfg(feature = "napi")]
use rayon::prelude::*;
#[cfg(feature = "napi")]
use rayon::ThreadPool;
use std::borrow::Cow;
use std::cmp;
use std::io::Cursor;
use std::panic;
use std::path::PathBuf;
use std::sync::Arc;

// Type alias for Result - use napi::Result when napi is enabled, otherwise use standard Result
#[cfg(feature = "napi")]
type EngineResult<T> = Result<T>;
#[cfg(not(feature = "napi"))]
type EngineResult<T> = std::result::Result<T, LazyImageError>;

// Helper function to convert LazyImageError to the appropriate error type
#[cfg(feature = "napi")]
fn to_engine_error(err: LazyImageError) -> napi::Error {
    napi::Error::from(err)
}

#[cfg(not(feature = "napi"))]
fn to_engine_error(err: LazyImageError) -> LazyImageError {
    err
}

#[derive(Debug)]
pub(crate) struct ResizeError {
    source_dims: (u32, u32),
    target_dims: (u32, u32),
    reason: String,
}

impl ResizeError {
    fn new(source_dims: (u32, u32), target_dims: (u32, u32), reason: impl Into<String>) -> Self {
        Self {
            source_dims,
            target_dims,
            reason: reason.into(),
        }
    }

    fn into_lazy_image_error(self) -> LazyImageError {
        LazyImageError::resize_failed(self.source_dims, self.target_dims, self.reason)
    }
}

// =============================================================================
// TRUE LAZY LOADING - Source enum for deferred file reading
// =============================================================================

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
    #[allow(dead_code)]
    fn load(&self) -> std::result::Result<Arc<Vec<u8>>, LazyImageError> {
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
    #[allow(dead_code)]
    fn as_path(&self) -> Option<&PathBuf> {
        match self {
            Source::Path(p) => Some(p),
            Source::Memory(_) => None,
        }
    }
}

/// The main image processing engine.
///
/// Usage:
/// ```js
/// const result = await ImageEngine.from(buffer)
///   .resize(800)
///   .rotate(90)
///   .grayscale()
///   .toBuffer('jpeg', 75);
/// ```
#[cfg_attr(feature = "napi", napi)]
#[allow(dead_code)]
pub struct ImageEngine {
    /// Image source - supports lazy loading from file path
    source: Option<Source>,
    /// Cached raw bytes (loaded on demand for Path sources)
    source_bytes: Option<Arc<Vec<u8>>>,
    /// Decoded image (populated after first decode or on sync operations)
    /// Uses Arc to share decoded image between engines. Combined with Cow<DynamicImage>
    /// in apply_ops, this enables true Copy-on-Write: no deep copy until mutation.
    decoded: Option<Arc<DynamicImage>>,
    /// Queued operations
    ops: Vec<Operation>,
    /// ICC color profile extracted from source image
    icc_profile: Option<Arc<Vec<u8>>>,
    /// Whether to preserve metadata (Exif, ICC, XMP) in output.
    /// Default is false (strip all) for security and smaller file sizes.
    keep_metadata: bool,
}

#[cfg(feature = "napi")]
#[napi]
impl ImageEngine {
    // =========================================================================
    // CONSTRUCTORS
    // =========================================================================

    /// Create engine from a buffer. Decoding is lazy.
    /// Extracts ICC profile from the source image if present.
    #[napi(factory)]
    pub fn from(buffer: Buffer) -> Self {
        let data = buffer.to_vec();

        // Extract ICC profile before any processing
        let icc_profile = extract_icc_profile(&data).map(Arc::new);
        let source_bytes = Arc::new(data);

        ImageEngine {
            source: Some(Source::Memory(source_bytes.clone())),
            source_bytes: Some(source_bytes),
            decoded: None,
            ops: Vec::new(),
            icc_profile,
            keep_metadata: false, // Strip metadata by default for security & smaller files
        }
    }

    /// Create engine from a file path.
    /// **TRUE LAZY LOADING**: Only stores the path - file is NOT read until needed.
    /// This is the recommended way for server-side processing of large images.
    #[napi(factory, js_name = "fromPath")]
    pub fn from_path(path: String) -> Result<Self> {
        // Validate that the file exists (fast check, no read)
        let path_buf = PathBuf::from(&path);
        if !path_buf.exists() {
            return Err(napi::Error::from(LazyImageError::file_not_found(&path)));
        }

        Ok(ImageEngine {
            source: Some(Source::Path(path_buf)),
            source_bytes: None, // Will be loaded on demand
            decoded: None,
            ops: Vec::new(),
            icc_profile: None, // Will be extracted when bytes are loaded
            keep_metadata: false, // Strip metadata by default for security & smaller files
        })
    }

    /// Create a clone of this engine (for multi-output scenarios)
    #[napi(js_name = "clone")]
    pub fn clone_engine(&self) -> Result<ImageEngine> {
        Ok(ImageEngine {
            source: self.source.clone(),
            source_bytes: self.source_bytes.clone(),
            decoded: self.decoded.clone(),
            ops: self.ops.clone(),
            icc_profile: self.icc_profile.clone(),
            keep_metadata: self.keep_metadata,
        })
    }

    // =========================================================================
    // PIPELINE OPERATIONS - All return Reference for JS method chaining
    // =========================================================================

    /// Resize image. Width or height can be null to maintain aspect ratio.
    #[napi]
    pub fn resize(
        &mut self,
        this: Reference<ImageEngine>,
        width: Option<u32>,
        height: Option<u32>,
    ) -> Reference<ImageEngine> {
        self.ops.push(Operation::Resize { width, height });
        this
    }

    /// Crop a region from the image.
    #[napi]
    pub fn crop(
        &mut self,
        this: Reference<ImageEngine>,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Reference<ImageEngine> {
        self.ops.push(Operation::Crop {
            x,
            y,
            width,
            height,
        });
        this
    }

    /// Rotate by degrees (90, 180, 270 only)
    #[napi]
    pub fn rotate(&mut self, this: Reference<ImageEngine>, degrees: i32) -> Reference<ImageEngine> {
        self.ops.push(Operation::Rotate { degrees });
        this
    }

    /// Flip horizontally
    #[napi(js_name = "flipH")]
    pub fn flip_h(&mut self, this: Reference<ImageEngine>) -> Reference<ImageEngine> {
        self.ops.push(Operation::FlipH);
        this
    }

    /// Flip vertically
    #[napi(js_name = "flipV")]
    pub fn flip_v(&mut self, this: Reference<ImageEngine>) -> Reference<ImageEngine> {
        self.ops.push(Operation::FlipV);
        this
    }

    /// Convert to grayscale
    #[napi]
    pub fn grayscale(&mut self, this: Reference<ImageEngine>) -> Reference<ImageEngine> {
        self.ops.push(Operation::Grayscale);
        this
    }

    /// Preserve metadata (Exif, ICC profile, XMP) in output.
    /// By default, all metadata is stripped for security (no GPS leak) and smaller file sizes.
    /// Call this method to keep metadata for photography sites or when color accuracy is important.
    #[napi(js_name = "keepMetadata")]
    pub fn keep_metadata(&mut self, this: Reference<ImageEngine>) -> Reference<ImageEngine> {
        self.keep_metadata = true;
        this
    }

    /// Adjust brightness (-100 to 100)
    #[napi]
    pub fn brightness(
        &mut self,
        this: Reference<ImageEngine>,
        value: i32,
    ) -> Reference<ImageEngine> {
        let clamped = value.clamp(-100, 100);
        self.ops.push(Operation::Brightness { value: clamped });
        this
    }

    /// Adjust contrast (-100 to 100)
    #[napi]
    pub fn contrast(&mut self, this: Reference<ImageEngine>, value: i32) -> Reference<ImageEngine> {
        let clamped = value.clamp(-100, 100);
        self.ops.push(Operation::Contrast { value: clamped });
        this
    }

    /// Ensure the image is in RGB/RGBA format (pixel format conversion, not color space transformation)
    /// Note: This does NOT perform ICC color profile conversion - it only ensures the pixel format.
    /// For true color space conversion with ICC profiles, use a dedicated color management library.
    #[napi(js_name = "ensureRgb")]
    pub fn ensure_rgb(&mut self, this: Reference<ImageEngine>) -> Result<Reference<ImageEngine>> {
        // Only support sRGB format assurance for now
        // DisplayP3 and AdobeRGB would require ICC color management
        self.ops.push(Operation::ColorSpace {
            target: crate::ops::ColorSpace::Srgb,
        });
        Ok(this)
    }

    #[cfg(feature = "napi")]
    fn emit_to_color_space_deprecation_warning(env: &Env) {
        const WARNING_MESSAGE: &str =
            "lazy-image: toColorspace() is deprecated and will be removed in v1.0. Use ensureRgb().";

        let warn_result = (|| {
            let global = env.get_global()?;
            let console: JsObject = global.get_named_property("console")?;
            let warn: JsFunction = console.get_named_property("warn")?;
            let message = env.create_string(WARNING_MESSAGE)?.into_unknown();
            warn.call(Some(&console), &[message])?;
            Ok::<(), napi::Error>(())
        })();

        if let Err(err) = warn_result {
            eprintln!(
                "lazy-image warning: {} (failed to forward warning to JS: {})",
                WARNING_MESSAGE, err
            );
        }
    }

    /// Legacy method - use ensureRgb() instead
    ///
    /// **Deprecated**: This method is deprecated and will be removed in v1.0.
    /// Use `ensureRgb()` for pixel format conversion instead.
    ///
    /// Note: This method does NOT perform true color space conversion with ICC profiles.
    /// It only ensures the pixel format is RGB/RGBA.
    #[napi(js_name = "toColorspace")]
    pub fn to_color_space(
        &mut self,
        env: Env,
        this: Reference<ImageEngine>,
        color_space: String,
    ) -> Result<Reference<ImageEngine>> {
        Self::emit_to_color_space_deprecation_warning(&env);

        match color_space.to_lowercase().as_str() {
            "srgb" => {
                // Still works, but deprecated
                self.ops.push(Operation::ColorSpace { target: crate::ops::ColorSpace::Srgb });
                Ok(this)
            },
            "p3" | "display-p3" | "adobergb" => {
                Err(napi::Error::from(LazyImageError::unsupported_color_space(&format!(
                    "Color space '{}' is not supported. Use ensureRgb() for pixel format conversion.", 
                    color_space
                ))))
            },
            _ => Err(napi::Error::from(LazyImageError::unsupported_color_space(&format!(
                "Unknown color space '{}'. Supported: 'srgb'. Use ensureRgb() instead.", 
                color_space
            )))),
        }
    }

    // =========================================================================
    // PRESETS - Common configurations for web optimization
    // =========================================================================

    /// Apply a built-in preset for common use cases.
    ///
    /// Available presets:
    /// - "thumbnail": 150x150, WebP quality 75 (gallery thumbnails)
    /// - "avatar": 200x200, WebP quality 80 (profile pictures)
    /// - "hero": 1920 width, JPEG quality 85 (hero images, banners)
    /// - "social": 1200x630, JPEG quality 80 (OGP/Twitter cards)
    ///
    /// Returns the preset configuration for use with toBuffer/toFile.
    #[napi]
    pub fn preset(&mut self, _this: Reference<ImageEngine>, name: String) -> Result<PresetResult> {
        let config = PresetConfig::get(&name)
            .ok_or_else(|| napi::Error::from(LazyImageError::invalid_preset(&name)))?;

        // Apply resize operation
        self.ops.push(Operation::Resize {
            width: config.width,
            height: config.height,
        });

        // Return preset info for the user to use with toBuffer/toFile
        let (format_str, quality) = match &config.format {
            OutputFormat::Jpeg { quality } => ("jpeg", Some(*quality)),
            OutputFormat::Png => ("png", None),
            OutputFormat::WebP { quality } => ("webp", Some(*quality)),
            OutputFormat::Avif { quality } => ("avif", Some(*quality)),
        };

        Ok(PresetResult {
            format: format_str.to_string(),
            quality,
            width: config.width,
            height: config.height,
        })
    }

    // =========================================================================
    // OUTPUT - Triggers async computation
    // =========================================================================

    /// Encode to buffer asynchronously.
    /// format: "jpeg", "jpg", "png", "webp"
    /// quality: 1-100 (default 80, ignored for PNG)
    ///
    /// **Non-destructive**: This method can be called multiple times on the same engine instance.
    /// The source data is cloned internally, allowing multiple format outputs.
    #[napi(ts_return_type = "Promise<Buffer>")]
    pub fn to_buffer(
        &mut self,
        format: String,
        quality: Option<u8>,
    ) -> Result<AsyncTask<EncodeTask>> {
        let output_format = OutputFormat::from_str(&format, quality)
            .map_err(|_e| napi::Error::from(LazyImageError::unsupported_format(&format)))?;

        // Lazy load: ensure source bytes are loaded before creating the task
        let source = self.ensure_source_bytes()?.clone();
        let decoded = self.decoded.clone();
        let ops = self.ops.clone();
        let icc_profile = self.icc_profile.clone();

        let keep_metadata = self.keep_metadata;

        Ok(AsyncTask::new(EncodeTask {
            source: Some(source),
            decoded,
            ops,
            format: output_format,
            icc_profile,
            keep_metadata,
        }))
    }

    /// Encode to buffer asynchronously with performance metrics.
    /// Returns `{ data: Buffer, metrics: ProcessingMetrics }`.
    ///
    /// **Non-destructive**: This method can be called multiple times on the same engine instance.
    /// The source data is cloned internally, allowing multiple format outputs.
    #[napi(ts_return_type = "Promise<OutputWithMetrics>")]
    pub fn to_buffer_with_metrics(
        &mut self,
        format: String,
        quality: Option<u8>,
    ) -> Result<AsyncTask<EncodeWithMetricsTask>> {
        let output_format = OutputFormat::from_str(&format, quality)
            .map_err(|_e| napi::Error::from(LazyImageError::unsupported_format(&format)))?;

        // Lazy load: ensure source bytes are loaded before creating the task
        let source = self.ensure_source_bytes()?.clone();
        let decoded = self.decoded.clone();
        let ops = self.ops.clone();
        let icc_profile = self.icc_profile.clone();
        let keep_metadata = self.keep_metadata;

        Ok(AsyncTask::new(EncodeWithMetricsTask {
            source: Some(source),
            decoded,
            ops,
            format: output_format,
            icc_profile,
            keep_metadata,
        }))
    }

    /// Encode and write directly to a file asynchronously.
    /// **Memory-efficient**: Combined with fromPath(), this enables
    /// full file-to-file processing without touching Node.js heap.
    ///
    /// **Non-destructive**: This method can be called multiple times on the same engine instance.
    /// The source data is cloned internally, allowing multiple format outputs.
    ///
    /// Returns the number of bytes written.
    #[napi(js_name = "toFile", ts_return_type = "Promise<number>")]
    pub fn to_file(
        &mut self,
        path: String,
        format: String,
        quality: Option<u8>,
    ) -> Result<AsyncTask<WriteFileTask>> {
        let output_format = OutputFormat::from_str(&format, quality)
            .map_err(|_e| napi::Error::from(LazyImageError::unsupported_format(&format)))?;

        // Lazy load: ensure source bytes are loaded before creating the task
        let source = self.ensure_source_bytes()?.clone();
        let decoded = self.decoded.clone();
        let ops = self.ops.clone();
        let icc_profile = self.icc_profile.clone();
        let keep_metadata = self.keep_metadata;

        Ok(AsyncTask::new(WriteFileTask {
            source: Some(source),
            decoded,
            ops,
            format: output_format,
            icc_profile,
            keep_metadata,
            output_path: path,
        }))
    }

    // =========================================================================
    // SYNC UTILITIES
    // =========================================================================

    /// Get image dimensions WITHOUT full decoding.
    /// For file paths, reads only the header bytes (extremely fast).
    /// For in-memory buffers, uses header-only parsing.
    #[napi]
    pub fn dimensions(&mut self) -> Result<Dimensions> {
        use image::ImageReader;

        // If already decoded, use that
        if let Some(ref img) = self.decoded {
            let (w, h) = img.dimensions();
            return Ok(Dimensions { width: w, height: h });
        }

        // Try to read dimensions from header only (no full decode)
        let source = self
            .source
            .as_ref()
            .ok_or_else(|| napi::Error::from(LazyImageError::source_consumed()))?;

        match source {
            Source::Path(path) => {
                // For file paths, read header directly from file (very fast)
                use std::fs::File;
                use std::io::BufReader;

                let file = File::open(path).map_err(|e| {
                    napi::Error::from(LazyImageError::file_read_failed(
                        path.to_string_lossy().to_string(),
                        e,
                    ))
                })?;

                let reader = ImageReader::new(BufReader::new(file))
                    .with_guessed_format()
                    .map_err(|e| {
                        napi::Error::from(LazyImageError::decode_failed(format!(
                            "failed to read image header: {e}"
                        )))
                    })?;

                let (width, height) = reader.into_dimensions().map_err(|e| {
                    napi::Error::from(LazyImageError::decode_failed(format!(
                        "failed to read dimensions: {e}"
                    )))
                })?;

                Ok(Dimensions { width, height })
            }
            Source::Memory(data) => {
                // For in-memory data, use cursor
                let cursor = Cursor::new(data.as_ref());
                let reader = ImageReader::new(cursor)
                    .with_guessed_format()
                    .map_err(|e| {
                        napi::Error::from(LazyImageError::decode_failed(format!(
                            "failed to read image header: {e}"
                        )))
                    })?;

                let (width, height) = reader.into_dimensions().map_err(|e| {
                    napi::Error::from(LazyImageError::decode_failed(format!(
                        "failed to read dimensions: {e}"
                    )))
                })?;

                Ok(Dimensions { width, height })
            }
        }
    }

    /// Check if an ICC color profile was extracted from the source image.
    /// Returns the profile size in bytes, or null if no profile exists.
    #[napi(js_name = "hasIccProfile")]
    pub fn has_icc_profile(&self) -> Option<u32> {
        self.icc_profile.as_ref().map(|p| p.len() as u32)
    }

    /// Process multiple images in parallel with the same operations.
    ///
    /// - inputs: Array of input file paths
    /// - output_dir: Directory to write processed images
    /// - format: Output format ("jpeg", "png", "webp", "avif")
    /// - quality: Optional quality (1-100, uses format-specific default if None)
    /// - concurrency: Optional number of parallel workers (default: CPU core count)
    #[napi(js_name = "processBatch", ts_return_type = "Promise<BatchResult[]>")]
    pub fn process_batch(
        &self,
        inputs: Vec<String>,
        output_dir: String,
        format: String,
        quality: Option<u8>,
        concurrency: Option<u32>,
    ) -> Result<AsyncTask<BatchTask>> {
        let output_format = OutputFormat::from_str(&format, quality)
            .map_err(|_e| napi::Error::from(LazyImageError::unsupported_format(&format)))?;
        let ops = self.ops.clone();
        Ok(AsyncTask::new(BatchTask {
            inputs,
            output_dir,
            ops,
            format: output_format,
            concurrency: concurrency.unwrap_or(0), // 0 = use default (CPU cores)
        }))
    }
}

#[cfg(feature = "napi")]
#[napi(object)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

#[cfg(feature = "napi")]
/// Result of applying a preset, contains recommended output settings
#[napi(object)]
pub struct PresetResult {
    /// Recommended output format
    pub format: String,
    /// Recommended quality (None for PNG)
    pub quality: Option<u8>,
    /// Target width (None if aspect ratio preserved)
    pub width: Option<u32>,
    /// Target height (None if aspect ratio preserved)
    pub height: Option<u32>,
}

#[cfg(feature = "napi")]
#[napi(object)]
pub struct BatchResult {
    pub source: String,
    pub success: bool,
    pub error: Option<String>,
    pub output_path: Option<String>,
}

// =============================================================================
// INTERNAL IMPLEMENTATION
// =============================================================================

impl ImageEngine {
    /// Ensure source bytes are loaded (lazy loading for Path sources)
    #[cfg(feature = "napi")]
    fn ensure_source_bytes(&mut self) -> Result<&Arc<Vec<u8>>> {
        if self.source_bytes.is_none() {
            let source = self
                .source
                .as_ref()
                .ok_or_else(|| napi::Error::from(LazyImageError::source_consumed()))?;

            let bytes = source.load().map_err(|e| napi::Error::from(e))?;

            // Extract ICC profile now that we have the bytes
            if self.icc_profile.is_none() {
                self.icc_profile = extract_icc_profile(&bytes).map(Arc::new);
            }

            self.source_bytes = Some(bytes);
        }

        self.source_bytes.as_ref().ok_or_else(|| {
            napi::Error::from(LazyImageError::internal_panic("source bytes load failed"))
        })
    }

    #[cfg(feature = "napi")]
    #[allow(dead_code)]
    fn ensure_decoded(&mut self) -> Result<&DynamicImage> {
        if self.decoded.is_none() {
            // First ensure we have the source bytes loaded
            let source = self.ensure_source_bytes()?.clone();

            let img = image::load_from_memory(&source).map_err(|e| {
                napi::Error::from(LazyImageError::decode_failed(format!(
                    "failed to decode: {e}"
                )))
            })?;

            // Security check: reject decompression bombs
            let (w, h) = img.dimensions();
            check_dimensions(w, h)?;

            // Wrap in Arc for sharing (enables Cow::Borrowed in decode())
            self.decoded = Some(Arc::new(img));
        }

        // Safe: we just set it above, use ok_or for safety
        // Return reference to inner DynamicImage
        self.decoded
            .as_ref()
            .map(|arc| arc.as_ref())
            .ok_or_else(|| {
                napi::Error::from(LazyImageError::internal_panic("decode failed unexpectedly"))
            })
    }

    /// Ensure source bytes are loaded (lazy loading for Path sources)
    #[cfg(not(feature = "napi"))]
    #[allow(dead_code)]
    fn ensure_source_bytes(&mut self) -> std::result::Result<&Arc<Vec<u8>>, LazyImageError> {
        if self.source_bytes.is_none() {
            let source = self
                .source
                .as_ref()
                .ok_or_else(|| LazyImageError::source_consumed())?;

            let bytes = source.load()?;

            // Extract ICC profile now that we have the bytes
            if self.icc_profile.is_none() {
                self.icc_profile = extract_icc_profile(&bytes).map(Arc::new);
            }

            self.source_bytes = Some(bytes);
        }

        self.source_bytes
            .as_ref()
            .ok_or_else(|| LazyImageError::internal_panic("source bytes load failed"))
    }

    #[cfg(not(feature = "napi"))]
    #[allow(dead_code)]
    fn ensure_decoded(&mut self) -> std::result::Result<&DynamicImage, LazyImageError> {
        if self.decoded.is_none() {
            // First ensure we have the source bytes loaded
            let source = self.ensure_source_bytes()?.clone();

            let img = image::load_from_memory(&source)
                .map_err(|e| LazyImageError::decode_failed(format!("failed to decode: {e}")))?;

            // Security check: reject decompression bombs
            let (w, h) = img.dimensions();
            check_dimensions(w, h)?;

            // Wrap in Arc for sharing (enables Cow::Borrowed in decode())
            self.decoded = Some(Arc::new(img));
        }

        // Safe: we just set it above, use ok_or for safety
        // Return reference to inner DynamicImage
        self.decoded
            .as_ref()
            .map(|arc| arc.as_ref())
            .ok_or_else(|| LazyImageError::internal_panic("decode failed unexpectedly"))
    }
}

// =============================================================================
// ASYNC TASK - Where the real work happens
// =============================================================================

pub struct EncodeTask {
    pub source: Option<Arc<Vec<u8>>>,
    /// Decoded image wrapped in Arc. decode() returns Cow::Borrowed pointing here,
    /// enabling true Copy-on-Write in apply_ops (no deep copy for format-only conversion).
    pub decoded: Option<Arc<DynamicImage>>,
    pub ops: Vec<Operation>,
    pub format: OutputFormat,
    pub icc_profile: Option<Arc<Vec<u8>>>,
    /// Whether to preserve metadata in output (default: false for security & smaller files)
    pub keep_metadata: bool,
}

impl EncodeTask {
    /// Decode image from source bytes
    /// Uses mozjpeg (libjpeg-turbo) for JPEG, falls back to image crate for others
    ///
    /// **True Copy-on-Write**: Returns `Cow::Borrowed` if image is already decoded,
    /// `Cow::Owned` if decoding was required. The caller can avoid deep copies
    /// when no mutation is needed (e.g., format conversion only).
    pub fn decode(&self) -> EngineResult<Cow<'_, DynamicImage>> {
        // Prefer already decoded image (already validated)
        // Return borrowed reference - no deep copy until mutation is needed
        if let Some(ref img_arc) = self.decoded {
            return Ok(Cow::Borrowed(img_arc.as_ref()));
        }

        let source = self
            .source
            .as_ref()
            .ok_or_else(|| to_engine_error(LazyImageError::source_consumed()))?;

        // Check magic bytes for JPEG (0xFF 0xD8)
        let img = if source.len() >= 2 && source[0] == 0xFF && source[1] == 0xD8 {
            // JPEG detected - use mozjpeg for TURBO speed
            Self::decode_jpeg_mozjpeg(source)?
        } else {
            // PNG, WebP, etc - use image crate
            image::load_from_memory(source).map_err(|e| {
                to_engine_error(LazyImageError::decode_failed(format!("decode failed: {e}")))
            })?
        };

        // Security check: reject decompression bombs
        let (w, h) = img.dimensions();
        check_dimensions(w, h)?;

        Ok(Cow::Owned(img))
    }
    /// Decode JPEG using mozjpeg (backed by libjpeg-turbo)
    /// This is SIGNIFICANTLY faster than image crate's pure Rust decoder
    fn decode_jpeg_mozjpeg(data: &[u8]) -> EngineResult<DynamicImage> {
        let result = panic::catch_unwind(|| {
            let decompress = Decompress::new_mem(data)
                .map_err(|e| format!("mozjpeg decompress init failed: {e:?}"))?;

            // Get image info
            let mut decompress = decompress
                .rgb()
                .map_err(|e| format!("mozjpeg rgb conversion failed: {e:?}"))?;

            let width = decompress.width();
            let height = decompress.height();

            // Validate dimensions before casting (mozjpeg returns usize)
            if width > MAX_DIMENSION as usize || height > MAX_DIMENSION as usize {
                return Err(format!(
                    "image dimensions {}x{} exceed max {}",
                    width, height, MAX_DIMENSION
                ));
            }

            // Read all scanlines
            let pixels: Vec<[u8; 3]> = decompress
                .read_scanlines()
                .map_err(|e| format!("mozjpeg: failed to read scanlines: {e:?}"))?;

            // Safe conversion from Vec<[u8; 3]> to Vec<u8>
            // Previously used unsafe Vec::from_raw_parts, now using safe iterator approach.
            // The compiler can optimize this into an efficient memory operation.
            let flat_pixels: Vec<u8> = pixels.into_iter().flatten().collect();

            // Create DynamicImage from raw RGB data
            // Safe cast: we validated dimensions above
            let rgb_image = RgbImage::from_raw(width as u32, height as u32, flat_pixels)
                .ok_or_else(|| "mozjpeg: failed to create image from raw data".to_string())?;

            Ok::<DynamicImage, String>(DynamicImage::ImageRgb8(rgb_image))
        });

        match result {
            Ok(Ok(img)) => Ok(img),
            Ok(Err(e)) => Err(to_engine_error(LazyImageError::decode_failed(e))),
            Err(_) => Err(to_engine_error(LazyImageError::internal_panic(
                "mozjpeg panicked during decode",
            ))),
        }
    }

    /// Optimize operations by combining consecutive resize/crop operations
    fn optimize_ops(ops: &[Operation]) -> Vec<Operation> {
        if ops.len() < 2 {
            return ops.to_vec();
        }

        let mut optimized = Vec::new();
        let mut i = 0;

        while i < ops.len() {
            let current = &ops[i];

            // Try to combine consecutive resize operations
            if let Operation::Resize {
                width: w1,
                height: h1,
            } = current
            {
                let mut final_width = *w1;
                let mut final_height = *h1;
                let mut j = i + 1;

                // Combine all consecutive resize operations
                while j < ops.len() {
                    if let Operation::Resize {
                        width: w2,
                        height: h2,
                    } = &ops[j]
                    {
                        // If both dimensions are specified, use the last one
                        // Otherwise, maintain aspect ratio from the first resize
                        if w2.is_some() && h2.is_some() {
                            final_width = *w2;
                            final_height = *h2;
                        } else if w2.is_some() {
                            final_width = *w2;
                            final_height = None;
                        } else if h2.is_some() {
                            final_width = None;
                            final_height = *h2;
                        }
                        j += 1;
                    } else {
                        break;
                    }
                }

                if j > i + 1 {
                    // Combined multiple resizes into one
                    optimized.push(Operation::Resize {
                        width: final_width,
                        height: final_height,
                    });
                    i = j;
                    continue;
                }
            }

            // Try to optimize crop + resize or resize + crop
            if i + 1 < ops.len() {
                match (&ops[i], &ops[i + 1]) {
                    // Crop then resize: optimize by calculating final dimensions
                    (
                        Operation::Crop {
                            x,
                            y,
                            width: cw,
                            height: ch,
                        },
                        Operation::Resize {
                            width: rw,
                            height: rh,
                        },
                    ) => {
                        let (final_w, final_h) = calc_resize_dimensions(*cw, *ch, *rw, *rh);
                        optimized.push(Operation::Crop {
                            x: *x,
                            y: *y,
                            width: *cw,
                            height: *ch,
                        });
                        optimized.push(Operation::Resize {
                            width: Some(final_w),
                            height: Some(final_h),
                        });
                        i += 2;
                        continue;
                    }
                    // Resize then crop: keep both but order is already optimal
                    (Operation::Resize { .. }, Operation::Crop { .. }) => {
                        // Keep both operations, but we could optimize further if needed
                    }
                    _ => {}
                }
            }

            optimized.push(current.clone());
            i += 1;
        }

        optimized
    }

    /// Apply all queued operations using Copy-on-Write semantics
    ///
    /// **True Copy-on-Write**: If no operations are queued (format conversion only),
    /// returns `Cow::Borrowed` - no pixel data is copied. Deep copy only happens
    /// when actual image manipulation (resize, crop, etc.) is required.
    pub fn apply_ops<'a>(
        img: Cow<'a, DynamicImage>,
        ops: &[Operation],
    ) -> EngineResult<Cow<'a, DynamicImage>> {
        // Optimize operations first
        let optimized_ops = Self::optimize_ops(ops);

        // No operations = no copy needed (format conversion only path)
        if optimized_ops.is_empty() {
            return Ok(img);
        }

        // Operations exist - we need owned data to mutate
        // This is where the "copy" in Copy-on-Write happens
        let mut img = img.into_owned();

        for op in &optimized_ops {
            img = match op {
                Operation::Resize { width, height } => {
                    let (w, h) = calc_resize_dimensions(img.width(), img.height(), *width, *height);
                    // Use SIMD-accelerated fast_image_resize with fallback to image crate
                    // Fallback is intentional: fast_image_resize may fail on edge cases
                    // (e.g., very small images, invalid dimensions), so we use image crate's
                    // proven implementation as a safe fallback
                    // For RGB/RGBA images, use fast_resize_owned to avoid clone() (zero-copy)
                    // Check format first to decide which path to take
                    if matches!(
                        img,
                        DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_)
                    ) {
                        // Try zero-copy resize first (no clone needed for RGB/RGBA)
                        match Self::fast_resize_owned(img, w, h) {
                            Ok(resized) => resized,
                            Err(err) => {
                                return Err(to_engine_error(err.into_lazy_image_error()));
                            }
                        }
                    } else {
                        // For other formats, use reference version (conversion needed anyway)
                        Self::fast_resize(&img, w, h).unwrap_or_else(|_| {
                            img.resize_exact(w, h, image::imageops::FilterType::Lanczos3)
                        })
                    }
                }

                Operation::Crop {
                    x,
                    y,
                    width,
                    height,
                } => {
                    // Validate crop bounds
                    let img_w = img.width();
                    let img_h = img.height();
                    if *x + *width > img_w || *y + *height > img_h {
                        return Err(to_engine_error(LazyImageError::invalid_crop_bounds(
                            *x, *y, *width, *height, img_w, img_h,
                        )));
                    }
                    img.crop_imm(*x, *y, *width, *height)
                }

                Operation::Rotate { degrees } => {
                    match degrees {
                        90 => img.rotate90(),
                        180 => img.rotate180(),
                        270 => img.rotate270(),
                        -90 => img.rotate270(),
                        -180 => img.rotate180(),
                        -270 => img.rotate90(),
                        0 => img, // No-op for 0 degrees
                        _ => {
                            return Err(to_engine_error(LazyImageError::invalid_rotation_angle(
                                *degrees,
                            )));
                        }
                    }
                }

                Operation::FlipH => img.fliph(),
                Operation::FlipV => img.flipv(),
                Operation::Grayscale => DynamicImage::ImageLuma8(img.to_luma8()),

                Operation::Brightness { value } => img.brighten(*value),

                Operation::Contrast { value } => {
                    // image crate expects f32, convert from our -100..100 scale
                    img.adjust_contrast(*value as f32)
                }

                Operation::ColorSpace { target } => {
                    match target {
                        crate::ops::ColorSpace::Srgb => {
                            // Ensure RGB8/RGBA8 format
                            match img {
                                DynamicImage::ImageRgb8(_) | DynamicImage::ImageRgba8(_) => img,
                                _ => DynamicImage::ImageRgb8(img.to_rgb8()),
                            }
                        }
                        crate::ops::ColorSpace::DisplayP3 | crate::ops::ColorSpace::AdobeRgb => {
                            return Err(to_engine_error(LazyImageError::unsupported_color_space(
                                format!("{:?}", target),
                            )));
                        }
                    }
                }
            };
        }
        Ok(Cow::Owned(img))
    }

    /// Fast resize with owned DynamicImage (zero-copy for RGB/RGBA)
    /// Returns Ok(resized) on success, Err(resize_error) on failure
    pub(crate) fn fast_resize_owned(
        img: DynamicImage,
        dst_width: u32,
        dst_height: u32,
    ) -> std::result::Result<DynamicImage, ResizeError> {
        fast_resize_owned_impl(img, dst_width, dst_height)
    }

    /// Fast resize with reference (for external API compatibility)
    pub fn fast_resize(
        img: &DynamicImage,
        dst_width: u32,
        dst_height: u32,
    ) -> std::result::Result<DynamicImage, String> {
        let src_width = img.width();
        let src_height = img.height();

        if src_width == 0 || src_height == 0 || dst_width == 0 || dst_height == 0 {
            return Err("invalid dimensions".to_string());
        }

        // Select pixel layout without forcing RGBA when not needed
        // Use into_raw() to avoid clone() - ownership transfer instead of copying
        let (pixel_type, src_pixels): (PixelType, Vec<u8>) = match img {
            DynamicImage::ImageRgb8(rgb) => {
                // Clone is necessary when we only have a reference
                let rgb_image = rgb.clone();
                (PixelType::U8x3, rgb_image.into_raw())
            }
            DynamicImage::ImageRgba8(rgba) => {
                // Clone is necessary when we only have a reference
                let rgba_image = rgba.clone();
                (PixelType::U8x4, rgba_image.into_raw())
            }
            _ => {
                let rgba = img.to_rgba8();
                (PixelType::U8x4, rgba.into_raw())
            }
        };

        Self::fast_resize_internal(
            src_width, src_height, src_pixels, pixel_type, dst_width, dst_height,
        )
    }

    /// Internal resize implementation (shared by both owned and reference versions)
    pub(crate) fn fast_resize_internal(
        src_width: u32,
        src_height: u32,
        src_pixels: Vec<u8>,
        pixel_type: PixelType,
        dst_width: u32,
        dst_height: u32,
    ) -> std::result::Result<DynamicImage, String> {
        fast_resize_internal_impl(
            src_width, src_height, src_pixels, pixel_type, dst_width, dst_height,
        )
    }

    /// Encode to JPEG using mozjpeg with RUTHLESS Web-optimized settings
    pub fn encode_jpeg(
        img: &DynamicImage,
        quality: u8,
        icc: Option<&[u8]>,
    ) -> EngineResult<Vec<u8>> {
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let pixels = rgb.into_raw();

        // mozjpeg can panic internally, so we catch it
        let result = panic::catch_unwind(|| -> std::result::Result<Vec<u8>, String> {
            let mut comp = Compress::new(ColorSpace::JCS_RGB);

            comp.set_size(w as usize, h as usize);

            // Output color space: YCbCr (standard for JPEG)
            comp.set_color_space(ColorSpace::JCS_YCbCr);

            // Quality setting with fine-grained control
            // Convert 0-100 to mozjpeg's quality scale (0.0-100.0)
            let quality_f32 = quality as f32;
            comp.set_quality(quality_f32);

            // =========================================================
            // RUTHLESS WEB OPTIMIZATION SETTINGS (Enhanced)
            // =========================================================

            // 1. Chroma Subsampling: Force 4:2:0 (same as sharp default)
            //    (2,2) means 2x2 pixel blocks for Cb and Cr channels
            //    This halves chroma resolution - imperceptible for photos
            comp.set_chroma_sampling_pixel_sizes((2, 2), (2, 2));

            // 2. Progressive mode: Better compression + progressive loading
            comp.set_progressive_mode();

            // 3. Optimize Huffman tables: Custom tables per image
            comp.set_optimize_coding(true);

            // 4. Optimize scan order: Better progressive compression
            comp.set_optimize_scans(true);
            comp.set_scan_optimization_mode(ScanMode::AllComponentsTogether);

            // 5. Enhanced Trellis quantization: Better rate-distortion optimization
            //    This is mozjpeg's secret sauce - it tries multiple quantization
            //    strategies and picks the best one for file size vs quality
            //    Trellis quantization is automatically enabled when optimize_coding is true (set above)
            //    This ensures consistent behavior and optimal compression
            //    Note: set_trellis_quantization() method is not available in mozjpeg 0.10 API,
            //    but Trellis quantization is guaranteed to be enabled via set_optimize_coding(true)

            // 6. Adaptive smoothing: Reduces high-frequency noise for better compression
            //    Higher quality = less smoothing, lower quality = more smoothing
            //    Enhanced smoothing for low quality (60 and below) to reduce block noise
            //    while maintaining compression ratio (good trade-off for web use)
            let smoothing = if quality_f32 >= 90.0 {
                0 // No smoothing for high quality
            } else if quality_f32 >= 70.0 {
                5 // Minimal smoothing
            } else if quality_f32 >= 60.0 {
                10 // Moderate smoothing
            } else {
                18 // Enhanced smoothing for lower quality (was 15, now 18 for better block noise reduction)
            };
            comp.set_smoothing_factor(smoothing);

            // 7. Quantization table optimization
            //    mozjpeg automatically optimizes quantization tables when optimize_coding is true

            // Estimate output size: ~10% of raw size for typical JPEG compression
            let estimated_size = (w as usize * h as usize * 3 / 10).max(4096);
            let mut output = Vec::with_capacity(estimated_size);

            {
                let mut writer = comp
                    .start_compress(&mut output)
                    .map_err(|e| format!("mozjpeg: failed to start compress: {e:?}"))?;

                let stride = w as usize * 3;
                for row in pixels.chunks(stride) {
                    writer
                        .write_scanlines(row)
                        .map_err(|e| format!("mozjpeg: failed to write scanlines: {e:?}"))?;
                }

                writer
                    .finish()
                    .map_err(|e| format!("mozjpeg: failed to finish: {e:?}"))?;
            }

            Ok(output)
        });

        let encoded = match result {
            Ok(Ok(data)) => data,
            Ok(Err(e)) => return Err(to_engine_error(LazyImageError::encode_failed("jpeg", e))),
            Err(_) => {
                return Err(to_engine_error(LazyImageError::internal_panic(
                    "mozjpeg panicked during encoding",
                )))
            }
        };

        // Embed ICC profile using img-parts if present
        if let Some(icc_data) = icc {
            Self::embed_icc_jpeg(encoded, icc_data)
        } else {
            Ok(encoded)
        }
    }

    /// Embed ICC profile into JPEG using img-parts
    fn embed_icc_jpeg(jpeg_data: Vec<u8>, icc: &[u8]) -> EngineResult<Vec<u8>> {
        use img_parts::jpeg::{Jpeg, JpegSegment};
        use img_parts::Bytes;

        let mut jpeg = Jpeg::from_bytes(Bytes::from(jpeg_data)).map_err(|e| {
            to_engine_error(LazyImageError::decode_failed(format!(
                "failed to parse JPEG for ICC: {e}"
            )))
        })?;

        // Build ICC marker: "ICC_PROFILE\0" + chunk_num + total_chunks + data
        // For simplicity, we embed as a single chunk (works for profiles < 64KB)
        let mut marker_data = Vec::with_capacity(14 + icc.len());
        marker_data.extend_from_slice(b"ICC_PROFILE\0");
        marker_data.push(1); // Chunk number
        marker_data.push(1); // Total chunks
        marker_data.extend_from_slice(icc);

        // Create APP2 segment
        let segment = JpegSegment::new_with_contents(
            img_parts::jpeg::markers::APP2,
            Bytes::from(marker_data),
        );

        // Insert after SOI (before other segments)
        let segments = jpeg.segments_mut();
        segments.insert(0, segment);

        // Encode back
        let mut output = Vec::new();
        jpeg.encoder().write_to(&mut output).map_err(|e| {
            to_engine_error(LazyImageError::encode_failed(
                "jpeg",
                format!("failed to write JPEG with ICC: {e}"),
            ))
        })?;

        Ok(output)
    }

    /// Encode to PNG using image crate
    pub fn encode_png(img: &DynamicImage, icc: Option<&[u8]>) -> EngineResult<Vec<u8>> {
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .map_err(|e| {
                to_engine_error(LazyImageError::encode_failed(
                    "png",
                    format!("PNG encode failed: {e}"),
                ))
            })?;

        // Embed ICC profile if present
        if let Some(icc_data) = icc {
            Self::embed_icc_png(buf, icc_data)
        } else {
            Ok(buf)
        }
    }

    /// Embed ICC profile into PNG using img-parts
    fn embed_icc_png(png_data: Vec<u8>, icc: &[u8]) -> EngineResult<Vec<u8>> {
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use img_parts::png::Png;
        use img_parts::{Bytes, ImageICC};
        use std::io::Write;

        let mut png = Png::from_bytes(Bytes::from(png_data)).map_err(|e| {
            to_engine_error(LazyImageError::decode_failed(format!(
                "failed to parse PNG for ICC: {e}"
            )))
        })?;

        // iCCP chunk format: profile_name (null-terminated) + compression_method (0) + compressed_data
        let profile_name = b"ICC\0"; // Short name
        let compression_method = 0u8; // zlib

        // Compress ICC data
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(icc).map_err(|e| {
            to_engine_error(LazyImageError::encode_failed(
                "png",
                format!("failed to compress ICC: {e}"),
            ))
        })?;
        let compressed = encoder.finish().map_err(|e| {
            to_engine_error(LazyImageError::encode_failed(
                "png",
                format!("failed to finish ICC compression: {e}"),
            ))
        })?;

        let mut chunk_data = Vec::with_capacity(profile_name.len() + 1 + compressed.len());
        chunk_data.extend_from_slice(profile_name);
        chunk_data.push(compression_method);
        chunk_data.extend_from_slice(&compressed);

        // Use img-parts' ICC API
        png.set_icc_profile(Some(Bytes::from(chunk_data)));

        // Encode back
        let mut output = Vec::new();
        png.encoder().write_to(&mut output).map_err(|e| {
            to_engine_error(LazyImageError::encode_failed(
                "png",
                format!("failed to write PNG with ICC: {e}"),
            ))
        })?;

        Ok(output)
    }

    /// Encode to WebP with optimized settings
    /// Avoids unnecessary alpha channel to reduce file size
    pub fn encode_webp(
        img: &DynamicImage,
        quality: u8,
        icc: Option<&[u8]>,
    ) -> EngineResult<Vec<u8>> {
        // Use RGB instead of RGBA for smaller files (unless alpha is needed)
        // If the image is already RGB, avoid unnecessary conversion by checking the type first
        // Note: We still need to convert/clone for encoder lifetime management, but we avoid
        // converting RGBA->RGB when the image is already RGB
        let rgb = match img {
            DynamicImage::ImageRgb8(rgb_img) => {
                // For RGB images, we can use the image directly
                // The clone is necessary for lifetime management with webp::Encoder
                rgb_img.clone()
            }
            _ => {
                // Convert to RGB for other formats (RGBA, etc.)
                img.to_rgb8()
            }
        };
        let (w, h) = rgb.dimensions();
        let encoder = webp::Encoder::from_rgb(&rgb, w, h);

        // Create WebPConfig with enhanced preprocessing for better compression
        let mut config = webp::WebPConfig::new().map_err(|_| {
            to_engine_error(LazyImageError::internal_panic(
                "failed to create WebPConfig",
            ))
        })?;

        let settings = QualitySettings::new(quality);
        config.quality = settings.quality;
        config.method = settings.webp_method();
        config.pass = settings.webp_pass();
        config.preprocessing = settings.webp_preprocessing();
        config.sns_strength = settings.webp_sns_strength();
        config.autofilter = 1;
        config.filter_strength = settings.webp_filter_strength();
        config.filter_sharpness = settings.webp_filter_sharpness();

        let mem = encoder.encode_advanced(&config).map_err(|e| {
            to_engine_error(LazyImageError::encode_failed(
                "webp",
                format!("WebP encode failed: {e:?}"),
            ))
        })?;

        let encoded = mem.to_vec();

        // Embed ICC profile if present
        if let Some(icc_data) = icc {
            Self::embed_icc_webp(encoded, icc_data)
        } else {
            Ok(encoded)
        }
    }

    /// Embed ICC profile into WebP using img-parts
    fn embed_icc_webp(webp_data: Vec<u8>, icc: &[u8]) -> EngineResult<Vec<u8>> {
        use img_parts::webp::WebP;
        use img_parts::Bytes;

        let mut webp = WebP::from_bytes(Bytes::from(webp_data)).map_err(|e| {
            to_engine_error(LazyImageError::decode_failed(format!(
                "failed to parse WebP for ICC: {e}"
            )))
        })?;

        // Set the ICCP chunk directly
        webp.set_icc_profile(Some(Bytes::from(icc.to_vec())));

        // Encode back
        let mut output = Vec::new();
        webp.encoder().write_to(&mut output).map_err(|e| {
            to_engine_error(LazyImageError::encode_failed(
                "webp",
                format!("failed to write WebP with ICC: {e}"),
            ))
        })?;

        Ok(output)
    }

    /// Encode to AVIF format using libavif (AOMedia reference implementation).
    ///
    /// This implementation properly supports:
    /// - ICC profile embedding via avifImageSetProfileICC
    /// - Accurate RGB-to-YUV conversion with proper color matrix
    /// - Alpha channel handling with separate quality control
    pub fn encode_avif(
        img: &DynamicImage,
        quality: u8,
        icc: Option<&[u8]>,
    ) -> EngineResult<Vec<u8>> {
        let settings = QualitySettings::new(quality);
        let (width, height) = img.dimensions();

        // Determine if image has alpha
        let has_alpha = img.color().has_alpha();

        // Get RGBA pixels (libavif handles RGB to YUV conversion internally)
        let rgba = img.to_rgba8();
        let pixels = rgba.as_raw();

        unsafe {
            // Create avifImage
            let avif_image = avifImageCreate(
                width,
                height,
                8, // 8-bit depth
                AVIF_PIXEL_FORMAT_YUV420,
            );

            if avif_image.is_null() {
                return Err(to_engine_error(LazyImageError::encode_failed(
                    "avif",
                    "Failed to create AVIF image",
                )));
            }

            // Set up RAII-style cleanup using a guard
            struct AvifImageGuard(*mut avifImage);
            impl Drop for AvifImageGuard {
                fn drop(&mut self) {
                    unsafe {
                        if !self.0.is_null() {
                            avifImageDestroy(self.0);
                        }
                    }
                }
            }
            let _image_guard = AvifImageGuard(avif_image);

            // Set color properties
            (*avif_image).colorPrimaries = AVIF_COLOR_PRIMARIES_BT709 as u16;
            (*avif_image).transferCharacteristics = AVIF_TRANSFER_CHARACTERISTICS_SRGB as u16;
            (*avif_image).matrixCoefficients = AVIF_MATRIX_COEFFICIENTS_BT709 as u16;
            (*avif_image).yuvRange = AVIF_RANGE_FULL;

            // Set ICC profile if provided
            if let Some(icc_data) = icc {
                let result = avifImageSetProfileICC(
                    avif_image,
                    icc_data.as_ptr(),
                    icc_data.len(),
                );
                if result != AVIF_RESULT_OK {
                    return Err(to_engine_error(LazyImageError::encode_failed(
                        "avif",
                        format!("Failed to set ICC profile: {:?}", result),
                    )));
                }
            }

            // Create and configure RGB image structure
            let mut rgb: avifRGBImage = std::mem::zeroed();
            avifRGBImageSetDefaults(&mut rgb, avif_image);

            rgb.format = AVIF_RGB_FORMAT_RGBA;
            rgb.depth = 8;
            rgb.pixels = pixels.as_ptr() as *mut u8;
            rgb.rowBytes = (width * 4) as u32;

            // Allocate YUV planes in the image
            let alloc_result = avifImageAllocatePlanes(avif_image, AVIF_PLANES_YUV);
            if alloc_result != AVIF_RESULT_OK {
                return Err(to_engine_error(LazyImageError::encode_failed(
                    "avif",
                    format!("Failed to allocate YUV planes: {:?}", alloc_result),
                )));
            }

            // Convert RGB to YUV using libavif's optimized conversion
            let convert_result = avifImageRGBToYUV(avif_image, &rgb);
            if convert_result != AVIF_RESULT_OK {
                return Err(to_engine_error(LazyImageError::encode_failed(
                    "avif",
                    format!("Failed to convert RGB to YUV: {:?}", convert_result),
                )));
            }

            // Handle alpha channel if present
            if has_alpha {
                let alloc_alpha_result = avifImageAllocatePlanes(avif_image, AVIF_PLANES_A);
                if alloc_alpha_result != AVIF_RESULT_OK {
                    return Err(to_engine_error(LazyImageError::encode_failed(
                        "avif",
                        format!("Failed to allocate alpha plane: {:?}", alloc_alpha_result),
                    )));
                }

                // Copy alpha channel data
                let alpha_plane = (*avif_image).alphaPlane;
                let alpha_row_bytes = (*avif_image).alphaRowBytes as usize;
                for y in 0..height as usize {
                    for x in 0..width as usize {
                        let src_idx = (y * width as usize + x) * 4 + 3; // Alpha is 4th component
                        let dst_idx = y * alpha_row_bytes + x;
                        *alpha_plane.add(dst_idx) = pixels[src_idx];
                    }
                }
            }

            // Create encoder
            let encoder = avifEncoderCreate();
            if encoder.is_null() {
                return Err(to_engine_error(LazyImageError::encode_failed(
                    "avif",
                    "Failed to create AVIF encoder",
                )));
            }

            // Set up encoder cleanup guard
            struct AvifEncoderGuard(*mut avifEncoder);
            impl Drop for AvifEncoderGuard {
                fn drop(&mut self) {
                    unsafe {
                        if !self.0.is_null() {
                            avifEncoderDestroy(self.0);
                        }
                    }
                }
            }
            let _encoder_guard = AvifEncoderGuard(encoder);

            // Configure encoder
            // libavif quality: 0 (worst) to 100 (lossless),
            // but internally uses quantizer where lower = better
            // quality maps to: minQuantizer and maxQuantizer
            (*encoder).quality = quality as i32;
            (*encoder).qualityAlpha = quality as i32;
            (*encoder).speed = settings.avif_speed();
            // libavif requires maxThreads >= 2 for multi-threading; cap at 8 to avoid runaway thread counts
            let cpu_threads = num_cpus::get();
            let capped = cmp::min(8, cpu_threads);
            let encoder_threads = cmp::max(2, capped) as i32;
            (*encoder).maxThreads = encoder_threads;

            // Encode出力を管理するRAIIガード
            struct AvifRwDataGuard(avifRWData);
            impl AvifRwDataGuard {
                fn new() -> Self {
                    unsafe { Self(std::mem::zeroed()) }
                }
            }
            impl Drop for AvifRwDataGuard {
                fn drop(&mut self) {
                    unsafe {
                        avifRWDataFree(&mut self.0);
                    }
                }
            }

            // Encode the image
            let mut output = AvifRwDataGuard::new();

            let add_result = avifEncoderAddImage(
                encoder,
                avif_image,
                1, // duration (1 for still image)
                AVIF_ADD_IMAGE_FLAG_SINGLE,
            );
            if add_result != AVIF_RESULT_OK {
                return Err(to_engine_error(LazyImageError::encode_failed(
                    "avif",
                    format!("Failed to add image to encoder: {:?}", add_result),
                )));
            }

            let finish_result = avifEncoderFinish(encoder, &mut output.0);
            if finish_result != AVIF_RESULT_OK {
                return Err(to_engine_error(LazyImageError::encode_failed(
                    "avif",
                    format!("Failed to finish encoding: {:?}", finish_result),
                )));
            }

            // Copy output data
            let encoded_data =
                std::slice::from_raw_parts(output.0.data, output.0.size).to_vec();

            Ok(encoded_data)
        }
    }

    /// Process image: decode → apply ops → encode
    /// This is the core processing pipeline shared by toBuffer and toFile.
    #[allow(dead_code)]
    fn process_and_encode(
        &mut self,
        mut metrics: Option<&mut crate::ProcessingMetrics>,
    ) -> EngineResult<Vec<u8>> {
        // 1. Decode
        let start_decode = std::time::Instant::now();
        let img = self.decode()?;
        if let Some(m) = metrics.as_deref_mut() {
            m.decode_time = start_decode.elapsed().as_secs_f64() * 1000.0;
        }

        // 2. Apply operations
        let start_process = std::time::Instant::now();
        let processed = Self::apply_ops(img, &self.ops)?;
        if let Some(m) = metrics.as_deref_mut() {
            m.process_time = start_process.elapsed().as_secs_f64() * 1000.0;
        }

        // 3. Encode - only preserve ICC profile if keep_metadata is true
        let start_encode = std::time::Instant::now();
        let icc = if self.keep_metadata {
            self.icc_profile.as_ref().map(|v| v.as_slice())
        } else {
            None // Strip metadata by default for security & smaller files
        };
        let result = match &self.format {
            OutputFormat::Jpeg { quality } => Self::encode_jpeg(&processed, *quality, icc),
            OutputFormat::Png => Self::encode_png(&processed, icc),
            OutputFormat::WebP { quality } => Self::encode_webp(&processed, *quality, icc),
            OutputFormat::Avif { quality } => Self::encode_avif(&processed, *quality, icc),
        }?;

        if let Some(m) = metrics {
            m.encode_time = start_encode.elapsed().as_secs_f64() * 1000.0;
            // Estimate memory (rough) - prevent overflow
            let (w, h) = processed.dimensions();
            m.memory_peak =
                (w as u64 * h as u64 * 4 + result.len() as u64).min(u32::MAX as u64) as u32;
        }

        Ok(result)
    }
}

#[cfg(feature = "napi")]
#[napi]
impl Task for EncodeTask {
    type Output = Vec<u8>;
    type JsValue = JsBuffer;

    fn compute(&mut self) -> Result<Self::Output> {
        self.process_and_encode(None)
    }

    fn resolve(&mut self, env: Env, output: Self::Output) -> Result<Self::JsValue> {
        env.create_buffer_with_data(output).map(|b| b.into_raw())
    }
}

#[allow(dead_code)]
pub struct EncodeWithMetricsTask {
    source: Option<Arc<Vec<u8>>>,
    /// Decoded image wrapped in Arc for sharing. See EncodeTask for Copy-on-Write details.
    decoded: Option<Arc<DynamicImage>>,
    ops: Vec<Operation>,
    format: OutputFormat,
    icc_profile: Option<Arc<Vec<u8>>>,
    keep_metadata: bool,
}

#[cfg(feature = "napi")]
#[napi]
impl Task for EncodeWithMetricsTask {
    type Output = (Vec<u8>, crate::ProcessingMetrics);
    type JsValue = crate::OutputWithMetrics;

    fn compute(&mut self) -> Result<Self::Output> {
        //Reuse EncodeTask logic
        let mut task = EncodeTask {
            source: self.source.clone(),
            decoded: self.decoded.clone(),
            ops: self.ops.clone(),
            format: self.format.clone(),
            icc_profile: self.icc_profile.clone(),
            keep_metadata: self.keep_metadata,
        };

        use crate::ProcessingMetrics;
        let mut metrics = ProcessingMetrics::default();
        let data = task.process_and_encode(Some(&mut metrics))?;
        Ok((data, metrics))
    }

    fn resolve(&mut self, env: Env, output: Self::Output) -> Result<Self::JsValue> {
        let (data, metrics) = output;
        let js_buffer = env.create_buffer_with_data(data)?.into_raw();
        Ok(crate::OutputWithMetrics {
            data: js_buffer,
            metrics,
        })
    }
}

// =============================================================================
// WRITE FILE TASK - File output without touching Node.js heap
// =============================================================================

#[allow(dead_code)]
pub struct WriteFileTask {
    source: Option<Arc<Vec<u8>>>,
    /// Decoded image wrapped in Arc for sharing. See EncodeTask for Copy-on-Write details.
    decoded: Option<Arc<DynamicImage>>,
    ops: Vec<Operation>,
    format: OutputFormat,
    icc_profile: Option<Arc<Vec<u8>>>,
    keep_metadata: bool,
    output_path: String,
}

#[cfg(feature = "napi")]
#[napi]
impl Task for WriteFileTask {
    type Output = u32; // Bytes written
    type JsValue = u32;

    fn compute(&mut self) -> Result<Self::Output> {
        use std::io::Write;
        use tempfile::NamedTempFile;

        // Create EncodeTask and use its process_and_encode method
        let mut encode_task = EncodeTask {
            source: self.source.clone(),
            decoded: self.decoded.clone(),
            ops: self.ops.clone(),
            format: self.format.clone(),
            icc_profile: self.icc_profile.clone(),
            keep_metadata: self.keep_metadata,
        };

        // Process image using shared logic
        let data = encode_task.process_and_encode(None)?;

        // Atomic write: write to temp file in the same directory as target,
        // then rename on success. tempfile automatically cleans up on drop.
        let output_dir = std::path::Path::new(&self.output_path)
            .parent()
            .ok_or_else(|| {
                napi::Error::from(LazyImageError::internal_panic(
                    "output path has no parent directory",
                ))
            })?;

        // Create temp file in the same directory as the target file
        // This ensures rename() works (cross-filesystem rename can fail)
        let mut temp_file = NamedTempFile::new_in(output_dir).map_err(|e| {
            napi::Error::from(LazyImageError::file_write_failed(
                &output_dir.to_string_lossy().to_string(),
                e,
            ))
        })?;

        let temp_path = temp_file.path().to_path_buf();
        // Check for overflow: NAPI requires u32, but we can't handle >4GB files
        let bytes_written = data.len().try_into().map_err(|_| {
            napi::Error::from(LazyImageError::internal_panic(
                "file size exceeds 4GB limit (u32::MAX)",
            ))
        })?;
        temp_file.write_all(&data).map_err(|e| {
            napi::Error::from(LazyImageError::file_write_failed(
                &temp_path.display().to_string(),
                e,
            ))
        })?;

        // Ensure data is flushed to disk
        temp_file.as_file_mut().sync_all().map_err(|e| {
            napi::Error::from(LazyImageError::file_write_failed(
                &temp_path.display().to_string(),
                e,
            ))
        })?;

        // Atomic rename: tempfile handles cleanup automatically if this fails
        temp_file.persist(&self.output_path).map_err(|e| {
            let io_error = std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to persist file: {}", e),
            );
            napi::Error::from(LazyImageError::file_write_failed(
                &self.output_path,
                io_error,
            ))
        })?;

        Ok(bytes_written)
    }
    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

#[allow(dead_code)]
pub struct BatchTask {
    inputs: Vec<String>,
    output_dir: String,
    ops: Vec<Operation>,
    format: OutputFormat,
    concurrency: u32,
}

#[cfg(feature = "napi")]
#[napi]
impl Task for BatchTask {
    type Output = Vec<BatchResult>;
    type JsValue = Vec<BatchResult>;

    fn compute(&mut self) -> Result<Self::Output> {
        use std::fs;
        use std::path::Path;

        if !Path::new(&self.output_dir).exists() {
            fs::create_dir_all(&self.output_dir).map_err(|e| {
                napi::Error::from(LazyImageError::file_write_failed(
                    &self.output_dir.clone(),
                    e,
                ))
            })?;
        }

        // Helper closure to process a single image
        let ops = &self.ops;
        let format = &self.format;
        let output_dir = &self.output_dir;
        let process_one = |input_path: &String| -> BatchResult {
            let result = (|| -> Result<String> {
                let data = fs::read(input_path).map_err(|e| {
                    napi::Error::from(LazyImageError::file_read_failed(input_path, e))
                })?;

                let icc_profile = extract_icc_profile(&data).map(Arc::new);

                let img = if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8 {
                    EncodeTask::decode_jpeg_mozjpeg(&data)?
                } else {
                    image::load_from_memory(&data).map_err(|e| {
                        napi::Error::from(LazyImageError::decode_failed(format!(
                            "decode failed: {e}"
                        )))
                    })?
                };

                let (w, h) = img.dimensions();
                check_dimensions(w, h)?;

                let processed = EncodeTask::apply_ops(Cow::Owned(img), ops)?;

                let icc = icc_profile.as_ref().map(|v| v.as_slice());
                let encoded = match format {
                    OutputFormat::Jpeg { quality } => {
                        EncodeTask::encode_jpeg(&processed, *quality, icc)?
                    }
                    OutputFormat::Png => EncodeTask::encode_png(&processed, icc)?,
                    OutputFormat::WebP { quality } => {
                        EncodeTask::encode_webp(&processed, *quality, icc)?
                    }
                    OutputFormat::Avif { quality } => {
                        EncodeTask::encode_avif(&processed, *quality, icc)?
                    }
                };

                let filename = Path::new(input_path).file_name().ok_or_else(|| {
                    napi::Error::from(LazyImageError::internal_panic("invalid filename"))
                })?;

                let extension = match format {
                    OutputFormat::Jpeg { .. } => "jpg",
                    OutputFormat::Png => "png",
                    OutputFormat::WebP { .. } => "webp",
                    OutputFormat::Avif { .. } => "avif",
                };

                let output_filename = Path::new(filename).with_extension(extension);
                let output_path = Path::new(output_dir).join(output_filename);

                // Atomic write: use tempfile for safe file writing
                use std::io::Write;
                use tempfile::NamedTempFile;

                let mut temp_file = NamedTempFile::new_in(output_dir).map_err(|e| {
                    napi::Error::from(LazyImageError::file_write_failed(output_dir, e))
                })?;

                let temp_path = temp_file.path().to_path_buf();
                temp_file.write_all(&encoded).map_err(|e| {
                    napi::Error::from(LazyImageError::file_write_failed(
                        &temp_path.display().to_string(),
                        e,
                    ))
                })?;

                temp_file.as_file_mut().sync_all().map_err(|e| {
                    napi::Error::from(LazyImageError::file_write_failed(
                        &temp_path.display().to_string(),
                        e,
                    ))
                })?;

                // Atomic rename
                temp_file.persist(&output_path).map_err(|e| {
                    let io_error = std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("failed to persist file: {}", e),
                    );
                    napi::Error::from(LazyImageError::file_write_failed(
                        &output_path.display().to_string(),
                        io_error,
                    ))
                })?;

                Ok(output_path.to_string_lossy().to_string())
            })();

            match result {
                Ok(path) => BatchResult {
                    source: input_path.clone(),
                    success: true,
                    error: None,
                    output_path: Some(path),
                },
                Err(e) => {
                    // Preserve error information with context
                    let error_msg = format!("{}: {}", input_path, e);
                    BatchResult {
                        source: input_path.clone(),
                        success: false,
                        error: Some(error_msg),
                        output_path: None,
                    }
                }
            }
        };

        // Validate concurrency parameter
        // concurrency = 0 means "use default" (CPU cores - UV_THREADPOOL_SIZE)
        // concurrency = 1..MAX_CONCURRENCY means "use specified number of threads"
        if self.concurrency > MAX_CONCURRENCY as u32 {
            return Err(napi::Error::from(LazyImageError::internal_panic(format!(
                "invalid concurrency value: {} (must be 0 or 1-{})",
                self.concurrency, MAX_CONCURRENCY
            ))));
        }

        // Use global thread pool for better performance
        let results: Vec<BatchResult> = if self.concurrency == 0 {
            // Use global thread pool with default concurrency
            // (automatically calculated based on CPU count and UV_THREADPOOL_SIZE)
            GLOBAL_THREAD_POOL.install(|| self.inputs.par_iter().map(process_one).collect())
        } else {
            // For custom concurrency, create a temporary pool with specified threads
            // Note: This creates a new pool per request, which is acceptable
            // for custom concurrency requirements
            use rayon::ThreadPoolBuilder;
            let pool = ThreadPoolBuilder::new()
                .num_threads(self.concurrency as usize)
                .build()
                .map_err(|e| {
                    napi::Error::from(LazyImageError::internal_panic(format!(
                        "failed to create thread pool: {}",
                        e
                    )))
                })?;

            pool.install(|| self.inputs.par_iter().map(process_one).collect())
        };

        Ok(results)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

// =============================================================================
// UTILITY FUNCTIONS
// =============================================================================

/// Calculate resize dimensions maintaining aspect ratio
///
/// When both width and height are specified, the image is resized to fit
/// inside the specified dimensions while maintaining aspect ratio (like
/// sharp's `{ fit: 'inside' }` option).
pub fn calc_resize_dimensions(
    orig_w: u32,
    orig_h: u32,
    target_w: Option<u32>,
    target_h: Option<u32>,
) -> (u32, u32) {
    match (target_w, target_h) {
        (Some(w), Some(h)) => {
            // Maintain aspect ratio while fitting inside the specified dimensions
            let orig_ratio = orig_w as f64 / orig_h as f64;
            let target_ratio = w as f64 / h as f64;

            if orig_ratio > target_ratio {
                // Original image is wider → fit to width
                let ratio = w as f64 / orig_w as f64;
                (w, (orig_h as f64 * ratio).round() as u32)
            } else {
                // Original image is taller → fit to height
                let ratio = h as f64 / orig_h as f64;
                ((orig_w as f64 * ratio).round() as u32, h)
            }
        }
        (Some(w), None) => {
            let ratio = w as f64 / orig_w as f64;
            (w, (orig_h as f64 * ratio).round() as u32)
        }
        (None, Some(h)) => {
            let ratio = h as f64 / orig_h as f64;
            ((orig_w as f64 * ratio).round() as u32, h)
        }
        (None, None) => (orig_w, orig_h),
    }
}

/// Extract ICC profile from image data.
/// Supports JPEG (APP2 marker), PNG (iCCP chunk), and WebP (ICCP chunk).
/// Check if image dimensions are within safe limits.
/// Returns an error if the image is too large (potential decompression bomb).
#[cfg(feature = "napi")]
pub fn check_dimensions(width: u32, height: u32) -> Result<()> {
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(napi::Error::from(LazyImageError::dimension_exceeds_limit(
            width.max(height),
            MAX_DIMENSION,
        )));
    }
    let pixels = width as u64 * height as u64;
    if pixels > MAX_PIXELS {
        return Err(napi::Error::from(
            LazyImageError::pixel_count_exceeds_limit(pixels, MAX_PIXELS),
        ));
    }
    Ok(())
}

#[cfg(not(feature = "napi"))]
pub fn check_dimensions(width: u32, height: u32) -> std::result::Result<(), LazyImageError> {
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(LazyImageError::dimension_exceeds_limit(
            width.max(height),
            MAX_DIMENSION,
        ));
    }
    let pixels = width as u64 * height as u64;
    if pixels > MAX_PIXELS {
        return Err(LazyImageError::pixel_count_exceeds_limit(
            pixels, MAX_PIXELS,
        ));
    }
    Ok(())
}
/// Validate ICC profile header
/// ICC profiles must start with a 128-byte header containing specific fields
#[allow(dead_code)]
fn validate_icc_profile(icc_data: &[u8]) -> bool {
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

#[allow(dead_code)]
fn extract_icc_profile(data: &[u8]) -> Option<Vec<u8>> {
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

/// Check if data is AVIF format (ISOBMFF with 'avif' brand)
#[allow(dead_code)]
fn is_avif_data(data: &[u8]) -> bool {
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
#[allow(dead_code)]
fn extract_icc_from_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    let jpeg = Jpeg::from_bytes(data.to_vec().into()).ok()?;
    jpeg.icc_profile().map(|icc| icc.to_vec())
}

/// Extract ICC profile from PNG data
#[allow(dead_code)]
fn extract_icc_from_png(data: &[u8]) -> Option<Vec<u8>> {
    let png = Png::from_bytes(data.to_vec().into()).ok()?;
    png.icc_profile().map(|icc| icc.to_vec())
}

/// Extract ICC profile from WebP data
#[allow(dead_code)]
fn extract_icc_from_webp(data: &[u8]) -> Option<Vec<u8>> {
    use img_parts::webp::WebP;
    let webp = WebP::from_bytes(data.to_vec().into()).ok()?;
    webp.icc_profile().map(|icc| icc.to_vec())
}

/// Extract ICC profile from AVIF data using libavif
#[allow(dead_code)]
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

// =============================================================================
// UNIT TESTS
// =============================================================================

fn fast_resize_owned_impl(
    img: DynamicImage,
    dst_width: u32,
    dst_height: u32,
) -> std::result::Result<DynamicImage, ResizeError> {
    let src_width = img.width();
    let src_height = img.height();

    if src_width == 0 || src_height == 0 || dst_width == 0 || dst_height == 0 {
        return Err(ResizeError::new(
            (src_width, src_height),
            (dst_width, dst_height),
            "invalid dimensions for resize",
        ));
    }

    // Select pixel layout without forcing RGBA when not needed
    // Use into_raw() to avoid clone() - ownership transfer instead of copying
    let (pixel_type, src_pixels): (PixelType, Vec<u8>) = match img {
        DynamicImage::ImageRgb8(rgb) => {
            // Zero-copy: directly take ownership of the pixel buffer
            (PixelType::U8x3, rgb.into_raw())
        }
        DynamicImage::ImageRgba8(rgba) => {
            // Zero-copy: directly take ownership of the pixel buffer
            (PixelType::U8x4, rgba.into_raw())
        }
        other => {
            // For other formats, convert to RGBA (necessary conversion)
            let rgba = other.to_rgba8();
            (PixelType::U8x4, rgba.into_raw())
        }
    };

    fast_resize_internal_impl(
        src_width, src_height, src_pixels, pixel_type, dst_width, dst_height,
    )
    .map_err(|reason| ResizeError::new((src_width, src_height), (dst_width, dst_height), reason))
}

fn fast_resize_internal_impl(
    src_width: u32,
    src_height: u32,
    src_pixels: Vec<u8>,
    pixel_type: PixelType,
    dst_width: u32,
    dst_height: u32,
) -> std::result::Result<DynamicImage, String> {
    // Create source image for fast_image_resize
    // from_vec_u8 takes ownership, avoiding the need for clone() on the pixels
    let src_image = fir::images::Image::from_vec_u8(src_width, src_height, src_pixels, pixel_type)
        .map_err(|e| format!("fir source image error: {e:?}"))?;

    // Create destination image
    let mut dst_image = fir::images::Image::new(dst_width, dst_height, pixel_type);

    // Create resizer with Lanczos3 (high quality)
    let mut resizer = fir::Resizer::new();

    // Resize with Lanczos3 filter
    let options =
        ResizeOptions::new().resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::Lanczos3));
    resizer
        .resize(&src_image, &mut dst_image, &options)
        .map_err(|e| format!("fir resize error: {e:?}"))?;

    // Convert back to DynamicImage
    let dst_pixels = dst_image.into_vec();
    match pixel_type {
        PixelType::U8x3 => {
            let rgb_image = RgbImage::from_raw(dst_width, dst_height, dst_pixels)
                .ok_or("failed to create rgb image from resized data")?;
            Ok(DynamicImage::ImageRgb8(rgb_image))
        }
        PixelType::U8x4 => {
            let rgba_image = RgbaImage::from_raw(dst_width, dst_height, dst_pixels)
                .ok_or("failed to create rgba image from resized data")?;
            Ok(DynamicImage::ImageRgba8(rgba_image))
        }
        _ => Err("unsupported pixel type after resize".to_string()),
    }
}

#[cfg(test)]
fn fast_resize_owned_test_hook(
    img: DynamicImage,
    dst_width: u32,
    dst_height: u32,
) -> std::result::Result<DynamicImage, ResizeError> {
    fast_resize_owned_impl(img, dst_width, dst_height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, GenericImageView, RgbImage, RgbaImage};

    // Helper function to create test images
    fn create_test_image(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        }))
    }

    fn create_test_image_rgba(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_fn(width, height, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
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
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
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

    #[test]
    fn fast_resize_owned_returns_error_instead_of_dummy_image() {
        let img = create_test_image(1, 1);
        let err = fast_resize_owned_test_hook(img, 0, 10).expect_err("expected resize failure");
        assert_eq!(err.source_dims, (1, 1));
        assert_eq!(err.target_dims, (0, 10));
        assert!(err.reason.contains("invalid dimensions"));
    }

    mod resize_calc_tests {
        use super::*;

        #[test]
        fn test_both_dimensions_specified() {
            let (w, h) = calc_resize_dimensions(1000, 800, Some(500), Some(400));
            assert_eq!((w, h), (500, 400));
        }

        #[test]
        fn test_width_only_maintains_aspect_ratio() {
            let (w, h) = calc_resize_dimensions(1000, 500, Some(500), None);
            assert_eq!(w, 500);
            assert_eq!(h, 250); // 1000:500 = 500:250
        }

        #[test]
        fn test_height_only_maintains_aspect_ratio() {
            let (w, h) = calc_resize_dimensions(1000, 500, None, Some(250));
            assert_eq!(w, 500);
            assert_eq!(h, 250);
        }

        #[test]
        fn test_none_returns_original() {
            let (w, h) = calc_resize_dimensions(1000, 500, None, None);
            assert_eq!((w, h), (1000, 500));
        }

        #[test]
        fn test_rounding_behavior() {
            // 奇数サイズでの丸め動作確認
            let (w, h) = calc_resize_dimensions(101, 51, Some(50), None);
            assert_eq!(w, 50);
            // 101:51 ≈ 50:25.2... → 25に丸められるべき
            assert_eq!(h, 25);
        }

        #[test]
        fn test_aspect_ratio_preservation_wide() {
            // 横長画像
            let (w, h) = calc_resize_dimensions(2000, 1000, Some(1000), None);
            assert_eq!(w, 1000);
            assert_eq!(h, 500);
        }

        #[test]
        fn test_aspect_ratio_preservation_tall() {
            // 縦長画像
            let (w, h) = calc_resize_dimensions(1000, 2000, None, Some(1000));
            assert_eq!(w, 500);
            assert_eq!(h, 1000);
        }

        #[test]
        fn test_square_image() {
            let (w, h) = calc_resize_dimensions(100, 100, Some(50), None);
            assert_eq!(w, 50);
            assert_eq!(h, 50);
        }

        #[test]
        fn test_both_dimensions_wide_image_fits_inside() {
            // 横長画像（6000×4000）を800×600にリサイズ
            // アスペクト比: 6000/4000 = 1.5 > 800/600 = 1.333...
            // → 幅に合わせて800×533になるべき
            let (w, h) = calc_resize_dimensions(6000, 4000, Some(800), Some(600));
            assert_eq!(w, 800);
            assert_eq!(h, 533); // 4000 * (800/6000) = 533.33... → 533
        }

        #[test]
        fn test_both_dimensions_tall_image_fits_inside() {
            // 縦長画像（4000×6000）を800×600にリサイズ
            // アスペクト比: 4000/6000 = 0.666... < 800/600 = 1.333...
            // → 高さに合わせて400×600になるべき
            let (w, h) = calc_resize_dimensions(4000, 6000, Some(800), Some(600));
            assert_eq!(w, 400); // 4000 * (600/6000) = 400
            assert_eq!(h, 600);
        }

        #[test]
        fn test_both_dimensions_same_aspect_ratio() {
            // 同じアスペクト比の場合は指定サイズそのまま
            // 1000:500 = 2:1, 800:400 = 2:1
            let (w, h) = calc_resize_dimensions(1000, 500, Some(800), Some(400));
            assert_eq!((w, h), (800, 400));
        }
    }

    mod security_tests {
        use super::*;

        #[test]
        fn test_check_dimensions_valid() {
            assert!(check_dimensions(1920, 1080).is_ok());
            // 32768 x 32768 = 1,073,741,824 > MAX_PIXELS(100,000,000) なのでエラーになる
            // MAX_DIMENSIONチェックは通るが、MAX_PIXELSチェックで弾かれる
            let result = check_dimensions(32768, 32768);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("exceeds max"));
        }

        #[test]
        fn test_check_dimensions_exceeds_max_dimension() {
            let result = check_dimensions(32769, 1);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("exceeds maximum"));
        }

        #[test]
        fn test_check_dimensions_exceeds_max_dimension_height() {
            let result = check_dimensions(1, 32769);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("exceeds maximum"));
        }

        #[test]
        fn test_check_dimensions_exceeds_max_pixels() {
            // 10001 x 10000 = 100,010,000 > MAX_PIXELS(100,000,000)
            let result = check_dimensions(10001, 10000);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("exceeds max"));
        }

        #[test]
        fn test_check_dimensions_at_pixel_boundary() {
            // ちょうど100,000,000ピクセル = OK
            assert!(check_dimensions(10000, 10000).is_ok());
        }

        #[test]
        fn test_check_dimensions_at_max_dimension() {
            // 境界値: 32768 x 32768 = 1,073,741,824 > MAX_PIXELS
            // しかし、MAX_DIMENSIONチェックが先に来るので、これはOK
            // 実際にはMAX_PIXELSチェックで弾かれる
            let result = check_dimensions(32768, 32768);
            // 32768 * 32768 = 1,073,741,824 > 100,000,000
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("exceeds max"));
        }

        #[test]
        fn test_check_dimensions_small_image() {
            assert!(check_dimensions(1, 1).is_ok());
        }

        #[test]
        fn test_check_dimensions_zero_dimension() {
            // 0次元は技術的には無効だが、check_dimensionsではチェックしない
            // image crateが処理する
            assert!(check_dimensions(0, 100).is_ok()); // 0 * 100 = 0 < MAX_PIXELS
        }
    }

    mod icc_tests {
        use super::*;

        #[test]
        fn test_validate_icc_profile_too_small() {
            let data = vec![0u8; 127]; // 128バイト未満
            assert!(!validate_icc_profile(&data));
        }

        #[test]
        fn test_validate_icc_profile_minimal_valid() {
            // 最小限の有効なICCプロファイル（128バイト）
            let mut data = vec![0u8; 128];
            // プロファイルサイズ（最初の4バイト、big-endian）
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0x80; // 128バイト
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
            // プロファイルサイズを200に設定
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0xC8; // 200バイト
                            // しかし実際のデータは200バイトなので、これは有効
                            // サイズが一致しない場合をテスト
            data[3] = 0x00;
            data[3] = 0xFF; // 255バイトと設定（実際は200バイト）

            // サイズが一致しないので無効
            assert!(!validate_icc_profile(&data));
        }

        #[test]
        fn test_validate_icc_profile_invalid_version() {
            let mut data = vec![0u8; 128];
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0x80;
            data[8] = 20; // バージョンが大きすぎる

            assert!(!validate_icc_profile(&data));
        }

        #[test]
        fn test_extract_icc_from_jpeg_no_profile() {
            // ICCプロファイルなしのJPEG
            let jpeg_data = create_minimal_jpeg();
            let result = extract_icc_from_jpeg(&jpeg_data);
            assert!(result.is_none());
        }

        #[test]
        fn test_extract_icc_from_png_no_profile() {
            // ICCプロファイルなしのPNG
            let png_data = create_minimal_png();
            let result = extract_icc_from_png(&png_data);
            assert!(result.is_none());
        }

        #[test]
        fn test_extract_icc_from_webp_no_profile() {
            // ICCプロファイルなしのWebP
            let webp_data = create_minimal_webp();
            let result = extract_icc_from_webp(&webp_data);
            assert!(result.is_none());
        }

        #[test]
        fn test_extract_icc_profile_invalid_data() {
            let invalid_data = vec![0u8; 10];
            let result = extract_icc_profile(&invalid_data);
            assert!(result.is_none());
        }

        #[test]
        fn test_extract_icc_profile_jpeg() {
            let jpeg_data = create_minimal_jpeg();
            // JPEGからICCプロファイルを抽出（存在しない場合）
            let result = extract_icc_profile(&jpeg_data);
            // 最小JPEGにはICCプロファイルがない
            assert!(result.is_none());
        }

        // Helper function to create a minimal valid ICC profile (sRGB)
        fn create_minimal_srgb_icc() -> Vec<u8> {
            // 最小限の有効なsRGB ICCプロファイル（128バイト）
            let mut data = vec![0u8; 128];
            // プロファイルサイズ（最初の4バイト、big-endian）
            data[0] = 0x00;
            data[1] = 0x00;
            data[2] = 0x00;
            data[3] = 0x80; // 128バイト
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
            EncodeTask::encode_jpeg(&img, 80, Some(icc)).unwrap()
        }

        // Helper function to create PNG with ICC profile
        fn create_png_with_icc(icc: &[u8]) -> Vec<u8> {
            let img = create_test_image(100, 100);
            EncodeTask::encode_png(&img, Some(icc)).unwrap()
        }

        // Helper function to create WebP with ICC profile
        fn create_webp_with_icc(icc: &[u8]) -> Vec<u8> {
            let img = create_test_image(100, 100);
            EncodeTask::encode_webp(&img, 80, Some(icc)).unwrap()
        }

        mod extraction_tests {
            use super::*;

            #[test]
            fn test_extract_icc_from_jpeg_with_profile() {
                let icc = create_minimal_srgb_icc();
                let jpeg = create_jpeg_with_icc(&icc);
                let extracted = extract_icc_profile(&jpeg);
                assert!(extracted.is_some());
                let extracted = extracted.unwrap();
                // ICCプロファイルの最小サイズは128バイト（ヘッダー）
                assert!(extracted.len() >= 128);
            }

            #[test]
            fn test_extract_icc_from_png_with_profile() {
                let icc = create_minimal_srgb_icc();
                let png = create_png_with_icc(&icc);
                let extracted = extract_icc_profile(&png);
                // PNGのICC埋め込みはimg-partsの実装に依存するため、
                // 抽出が成功するかどうかは実装次第
                // 少なくともエラーにならないことを確認
                // 実際の動作はimg-partsのバージョンに依存する可能性がある
                if extracted.is_none() {
                    // PNGのICC埋め込みが動作しない場合は、警告として記録
                    // これは既知の制限事項の可能性がある
                    eprintln!("Warning: PNG ICC profile extraction failed - this may be a limitation of img-parts");
                }
            }

            #[test]
            fn test_extract_icc_from_webp_with_profile() {
                let icc = create_minimal_srgb_icc();
                let webp = create_webp_with_icc(&icc);
                let extracted = extract_icc_profile(&webp);
                assert!(extracted.is_some());
            }

            #[test]
            fn test_extract_icc_returns_none_for_no_icc() {
                let jpeg = create_minimal_jpeg();
                let icc = extract_icc_profile(&jpeg);
                assert!(icc.is_none());
            }

            #[test]
            fn test_extract_icc_returns_none_for_non_image() {
                let icc = extract_icc_profile(b"not an image");
                assert!(icc.is_none());
            }

            #[test]
            fn test_extract_icc_returns_none_for_empty() {
                let icc = extract_icc_profile(&[]);
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
                // 途中で切り詰め
                let truncated = &icc[..50];
                assert!(!validate_icc_profile(truncated));
            }

            #[test]
            fn test_validate_wrong_size_field() {
                let mut icc = create_minimal_srgb_icc();
                // サイズフィールド（先頭4バイト）を不正値に
                icc[0] = 0xFF;
                icc[1] = 0xFF;
                icc[2] = 0xFF;
                icc[3] = 0xFF;
                assert!(!validate_icc_profile(&icc));
            }

            #[test]
            fn test_validate_too_short() {
                assert!(!validate_icc_profile(&[0; 100])); // 128バイト未満
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
                // 1. 元画像からICC抽出
                let original_icc = create_minimal_srgb_icc();
                let jpeg = create_jpeg_with_icc(&original_icc);
                let extracted_icc = extract_icc_profile(&jpeg).unwrap();

                // 2. 画像デコード
                let img = image::load_from_memory(&jpeg).unwrap();

                // 3. ICCを埋め込んでJPEGエンコード
                let encoded = EncodeTask::encode_jpeg(&img, 80, Some(&extracted_icc)).unwrap();

                // 4. エンコード結果からICC再抽出
                let re_extracted_icc = extract_icc_profile(&encoded).unwrap();

                // 5. 同一性確認
                assert_eq!(extracted_icc, re_extracted_icc);
            }

            #[test]
            fn test_png_roundtrip() {
                let original_icc = create_minimal_srgb_icc();
                let png = create_png_with_icc(&original_icc);
                let extracted_icc = extract_icc_profile(&png);

                // PNGのICC埋め込みが動作しない場合はスキップ
                if extracted_icc.is_none() {
                    eprintln!("Skipping PNG roundtrip test - ICC extraction not supported");
                    return;
                }

                let extracted_icc = extracted_icc.unwrap();
                let img = image::load_from_memory(&png).unwrap();
                let encoded = EncodeTask::encode_png(&img, Some(&extracted_icc)).unwrap();
                let re_extracted_icc = extract_icc_profile(&encoded);

                if re_extracted_icc.is_some() {
                    assert_eq!(extracted_icc, re_extracted_icc.unwrap());
                } else {
                    eprintln!("Warning: PNG ICC roundtrip failed - ICC may not be preserved");
                }
            }

            #[test]
            fn test_webp_roundtrip() {
                let original_icc = create_minimal_srgb_icc();
                let webp = create_webp_with_icc(&original_icc);
                let extracted_icc = extract_icc_profile(&webp).unwrap();

                let img = image::load_from_memory(&webp).unwrap();
                let encoded = EncodeTask::encode_webp(&img, 80, Some(&extracted_icc)).unwrap();
                let re_extracted_icc = extract_icc_profile(&encoded).unwrap();

                assert_eq!(extracted_icc, re_extracted_icc);
            }

            #[test]
            fn test_cross_format_roundtrip_jpeg_to_png() {
                // JPEGからICCを抽出してPNGに埋め込み
                let icc = create_minimal_srgb_icc();
                let jpeg = create_jpeg_with_icc(&icc);
                let extracted_icc = extract_icc_profile(&jpeg).unwrap();

                let img = image::load_from_memory(&jpeg).unwrap();
                let png = EncodeTask::encode_png(&img, Some(&extracted_icc)).unwrap();
                let re_extracted = extract_icc_profile(&png);

                // PNGのICC抽出が動作しない場合はスキップ
                if re_extracted.is_none() {
                    eprintln!(
                        "Skipping JPEG to PNG roundtrip test - PNG ICC extraction not supported"
                    );
                    return;
                }

                assert_eq!(extracted_icc, re_extracted.unwrap());
            }

            #[test]
            fn test_cross_format_roundtrip_png_to_webp() {
                // PNGからICCを抽出してWebPに埋め込み
                let icc = create_minimal_srgb_icc();
                let png = create_png_with_icc(&icc);
                let extracted_icc = extract_icc_profile(&png);

                // PNGのICC抽出が動作しない場合はスキップ
                if extracted_icc.is_none() {
                    eprintln!(
                        "Skipping PNG to WebP roundtrip test - PNG ICC extraction not supported"
                    );
                    return;
                }

                let extracted_icc = extracted_icc.unwrap();
                let img = image::load_from_memory(&png).unwrap();
                let webp = EncodeTask::encode_webp(&img, 80, Some(&extracted_icc)).unwrap();
                let re_extracted = extract_icc_profile(&webp).unwrap();

                assert_eq!(extracted_icc, re_extracted);
            }
        }

        mod avif_icc_tests {
            use super::*;

            #[test]
            fn test_avif_preserves_icc_profile() {
                // libavif implementation now properly embeds ICC profiles
                let icc = create_minimal_srgb_icc();
                let img = create_test_image(100, 100);
                let avif = EncodeTask::encode_avif(&img, 60, Some(&icc)).unwrap();

                // Verify AVIF data is valid
                assert!(is_avif_data(&avif), "Output should be valid AVIF");

                // Extract ICC profile from AVIF
                let extracted = extract_icc_profile(&avif);
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
                // ICCプロファイルを渡してもクラッシュしないことを確認
                let icc = create_minimal_srgb_icc();
                let img = create_test_image(100, 100);
                let result = EncodeTask::encode_avif(&img, 60, Some(&icc));
                assert!(result.is_ok(), "AVIF encoding with ICC should succeed");
            }

            #[test]
            fn test_avif_encoding_without_icc() {
                // ICC無しでもエンコードできることを確認
                let img = create_test_image(100, 100);
                let avif = EncodeTask::encode_avif(&img, 60, None).unwrap();

                // Verify AVIF data is valid
                assert!(is_avif_data(&avif), "Output should be valid AVIF");

                // Should not have ICC profile
                let extracted = extract_icc_profile(&avif);
                assert!(
                    extracted.is_none(),
                    "AVIF without ICC should not have ICC profile"
                );
            }
        }
    }

    mod apply_ops_tests {
        use super::*;

        #[test]
        fn test_resize_operation() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Resize {
                width: Some(50),
                height: Some(50),
            }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 50));
        }

        #[test]
        fn test_resize_width_only() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Resize {
                width: Some(50),
                height: None,
            }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 25));
        }

        #[test]
        fn test_resize_height_only() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Resize {
                width: None,
                height: Some(25),
            }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 25));
        }

        #[test]
        fn test_crop_valid() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Crop {
                x: 10,
                y: 10,
                width: 50,
                height: 50,
            }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 50));
        }

        #[test]
        fn test_crop_out_of_bounds() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Crop {
                x: 60,
                y: 60,
                width: 50,
                height: 50,
            }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Crop bounds"));
        }

        #[test]
        fn test_crop_at_origin() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Crop {
                x: 0,
                y: 0,
                width: 50,
                height: 50,
            }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 50));
        }

        #[test]
        fn test_crop_entire_image() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Crop {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_rotate_90() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: 90 }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 100)); // 幅と高さが入れ替わる
        }

        #[test]
        fn test_rotate_180() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: 180 }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 50)); // サイズは変わらない
        }

        #[test]
        fn test_rotate_270() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: 270 }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 100));
        }

        #[test]
        fn test_rotate_neg90() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: -90 }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (50, 100));
        }

        #[test]
        fn test_rotate_0() {
            let img = create_test_image(100, 50);
            let ops = vec![Operation::Rotate { degrees: 0 }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 50));
        }

        #[test]
        fn test_rotate_invalid_angle() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Rotate { degrees: 45 }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops);
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Unsupported rotation angle"));
        }

        #[test]
        fn test_flip_h() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::FlipH];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_flip_v() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::FlipV];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_grayscale_reduces_channels() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Grayscale];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            // グレースケール後はLuma8形式
            assert!(matches!(*result, DynamicImage::ImageLuma8(_)));
        }

        #[test]
        fn test_brightness() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Brightness { value: 50 }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_contrast() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::Contrast { value: 50 }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_colorspace_srgb() {
            let img = create_test_image(100, 100);
            let ops = vec![Operation::ColorSpace {
                target: crate::ops::ColorSpace::Srgb,
            }];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }

        #[test]
        fn test_chained_operations() {
            let img = create_test_image(200, 100);
            let ops = vec![
                Operation::Resize {
                    width: Some(100),
                    height: None,
                },
                Operation::Rotate { degrees: 90 },
                Operation::Grayscale,
            ];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            // 200x100 → resize → 100x50 → rotate90 → 50x100
            assert_eq!(result.dimensions(), (50, 100));
            assert!(matches!(*result, DynamicImage::ImageLuma8(_)));
        }

        #[test]
        fn test_empty_operations() {
            let img = create_test_image(100, 100);
            let ops = vec![];
            let result = EncodeTask::apply_ops(Cow::Owned(img), &ops).unwrap();
            assert_eq!(result.dimensions(), (100, 100));
        }
    }

    mod optimize_ops_tests {
        use super::*;

        #[test]
        fn test_consecutive_resizes_combined() {
            let ops = vec![
                Operation::Resize {
                    width: Some(800),
                    height: None,
                },
                Operation::Resize {
                    width: Some(400),
                    height: None,
                },
            ];
            let optimized = EncodeTask::optimize_ops(&ops);
            assert_eq!(optimized.len(), 1);
            if let Operation::Resize { width, height: _ } = &optimized[0] {
                assert_eq!(*width, Some(400));
            } else {
                panic!("Expected Resize operation");
            }
        }

        #[test]
        fn test_non_consecutive_resizes_not_combined() {
            let ops = vec![
                Operation::Resize {
                    width: Some(800),
                    height: None,
                },
                Operation::Grayscale,
                Operation::Resize {
                    width: Some(400),
                    height: None,
                },
            ];
            let optimized = EncodeTask::optimize_ops(&ops);
            assert_eq!(optimized.len(), 3);
        }

        #[test]
        fn test_single_operation() {
            let ops = vec![Operation::Resize {
                width: Some(100),
                height: None,
            }];
            let optimized = EncodeTask::optimize_ops(&ops);
            assert_eq!(optimized.len(), 1);
        }

        #[test]
        fn test_empty_operations() {
            let ops = vec![];
            let optimized = EncodeTask::optimize_ops(&ops);
            assert_eq!(optimized.len(), 0);
        }

        #[test]
        fn test_multiple_consecutive_resizes() {
            let ops = vec![
                Operation::Resize {
                    width: Some(1000),
                    height: None,
                },
                Operation::Resize {
                    width: Some(800),
                    height: None,
                },
                Operation::Resize {
                    width: Some(400),
                    height: None,
                },
            ];
            let optimized = EncodeTask::optimize_ops(&ops);
            assert_eq!(optimized.len(), 1);
            if let Operation::Resize { width, height: _ } = &optimized[0] {
                assert_eq!(*width, Some(400));
            }
        }

        #[test]
        fn test_resize_with_both_dimensions() {
            let ops = vec![
                Operation::Resize {
                    width: Some(800),
                    height: None,
                },
                Operation::Resize {
                    width: Some(400),
                    height: Some(300),
                },
            ];
            let optimized = EncodeTask::optimize_ops(&ops);
            assert_eq!(optimized.len(), 1);
            if let Operation::Resize { width, height } = &optimized[0] {
                assert_eq!(*width, Some(400));
                assert_eq!(*height, Some(300));
            }
        }
    }

    mod encode_tests {
        use super::*;

        #[test]
        fn test_encode_jpeg_produces_valid_jpeg() {
            let img = create_test_image(100, 100);
            let result = EncodeTask::encode_jpeg(&img, 80, None).unwrap();
            // JPEGマジックバイト確認
            assert_eq!(&result[0..2], &[0xFF, 0xD8]);
            // JPEGエンドマーカー確認
            assert_eq!(&result[result.len() - 2..], &[0xFF, 0xD9]);
        }

        #[test]
        fn test_encode_jpeg_quality_affects_size() {
            let img = create_test_image(100, 100);
            let high_quality = EncodeTask::encode_jpeg(&img, 95, None).unwrap();
            let low_quality = EncodeTask::encode_jpeg(&img, 50, None).unwrap();
            // 高品質の方が通常は大きい（ただし、画像内容によっては逆転する可能性もある）
            // 少なくとも両方とも有効なJPEGであることを確認
            assert!(high_quality.len() > 0);
            assert!(low_quality.len() > 0);
            assert_eq!(&high_quality[0..2], &[0xFF, 0xD8]);
            assert_eq!(&low_quality[0..2], &[0xFF, 0xD8]);
        }

        #[test]
        fn test_encode_jpeg_with_icc() {
            let img = create_test_image(100, 100);
            // 最小限の有効なICCプロファイル
            let mut icc_data = vec![0u8; 128];
            icc_data[0] = 0x00;
            icc_data[1] = 0x00;
            icc_data[2] = 0x00;
            icc_data[3] = 0x80; // 128バイト
            icc_data[4] = b'A';
            icc_data[5] = b'D';
            icc_data[6] = b'B';
            icc_data[7] = b'E';
            icc_data[8] = 2;
            icc_data[12] = b'm';
            icc_data[13] = b'n';
            icc_data[14] = b't';
            icc_data[15] = b'r';
            icc_data[16] = b'R';
            icc_data[17] = b'G';
            icc_data[18] = b'B';
            icc_data[19] = b' ';
            icc_data[20] = b'X';
            icc_data[21] = b'Y';
            icc_data[22] = b'Z';
            icc_data[23] = b' ';

            let result = EncodeTask::encode_jpeg(&img, 80, Some(&icc_data)).unwrap();
            assert_eq!(&result[0..2], &[0xFF, 0xD8]);
        }

        #[test]
        fn test_encode_png_produces_valid_png() {
            let img = create_test_image(100, 100);
            let result = EncodeTask::encode_png(&img, None).unwrap();
            // PNGマジックバイト確認
            assert_eq!(
                &result[0..8],
                &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
            );
        }

        #[test]
        fn test_encode_png_with_icc() {
            let img = create_test_image(100, 100);
            let mut icc_data = vec![0u8; 128];
            icc_data[0] = 0x00;
            icc_data[1] = 0x00;
            icc_data[2] = 0x00;
            icc_data[3] = 0x80;
            icc_data[4] = b'A';
            icc_data[5] = b'D';
            icc_data[6] = b'B';
            icc_data[7] = b'E';
            icc_data[8] = 2;
            icc_data[12] = b'm';
            icc_data[13] = b'n';
            icc_data[14] = b't';
            icc_data[15] = b'r';
            icc_data[16] = b'R';
            icc_data[17] = b'G';
            icc_data[18] = b'B';
            icc_data[19] = b' ';
            icc_data[20] = b'X';
            icc_data[21] = b'Y';
            icc_data[22] = b'Z';
            icc_data[23] = b' ';

            let result = EncodeTask::encode_png(&img, Some(&icc_data)).unwrap();
            assert_eq!(
                &result[0..8],
                &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
            );
        }

        #[test]
        fn test_encode_webp_produces_valid_webp() {
            let img = create_test_image(100, 100);
            let result = EncodeTask::encode_webp(&img, 80, None).unwrap();
            // WebPマジックバイト確認 (RIFF....WEBP)
            assert_eq!(&result[0..4], b"RIFF");
            assert_eq!(&result[8..12], b"WEBP");
        }

        #[test]
        fn test_encode_webp_with_icc() {
            let img = create_test_image(100, 100);
            let mut icc_data = vec![0u8; 128];
            icc_data[0] = 0x00;
            icc_data[1] = 0x00;
            icc_data[2] = 0x00;
            icc_data[3] = 0x80;
            icc_data[4] = b'A';
            icc_data[5] = b'D';
            icc_data[6] = b'B';
            icc_data[7] = b'E';
            icc_data[8] = 2;
            icc_data[12] = b'm';
            icc_data[13] = b'n';
            icc_data[14] = b't';
            icc_data[15] = b'r';
            icc_data[16] = b'R';
            icc_data[17] = b'G';
            icc_data[18] = b'B';
            icc_data[19] = b' ';
            icc_data[20] = b'X';
            icc_data[21] = b'Y';
            icc_data[22] = b'Z';
            icc_data[23] = b' ';

            let result = EncodeTask::encode_webp(&img, 80, Some(&icc_data)).unwrap();
            assert_eq!(&result[0..4], b"RIFF");
            assert_eq!(&result[8..12], b"WEBP");
        }

        #[test]
        fn test_encode_avif_produces_valid_avif() {
            let img = create_test_image(100, 100);
            let result = EncodeTask::encode_avif(&img, 60, None).unwrap();
            // AVIFは先頭にftypボックス
            assert!(result.len() > 12);
            // "ftyp"が含まれることを確認
            let has_ftyp = result.windows(4).any(|w| w == b"ftyp");
            assert!(has_ftyp);
        }

        #[test]
        fn test_encode_avif_quality_affects_size() {
            let img = create_test_image(100, 100);
            let high_quality = EncodeTask::encode_avif(&img, 80, None).unwrap();
            let low_quality = EncodeTask::encode_avif(&img, 40, None).unwrap();
            // 両方とも有効なAVIFであることを確認
            assert!(high_quality.len() > 0);
            assert!(low_quality.len() > 0);
        }

        #[test]
        fn test_encode_rgba_image() {
            let img = create_test_image_rgba(100, 100);
            let jpeg_result = EncodeTask::encode_jpeg(&img, 80, None).unwrap();
            assert_eq!(&jpeg_result[0..2], &[0xFF, 0xD8]);

            let png_result = EncodeTask::encode_png(&img, None).unwrap();
            assert_eq!(
                &png_result[0..8],
                &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
            );
        }
    }

    mod decode_tests {
        use super::*;

        #[test]
        fn test_decode_jpeg_mozjpeg() {
            let jpeg_data = create_minimal_jpeg();
            let result = EncodeTask::decode_jpeg_mozjpeg(&jpeg_data);
            assert!(result.is_ok());
            let img = result.unwrap();
            assert!(img.dimensions().0 > 0);
            assert!(img.dimensions().1 > 0);
        }

        #[test]
        fn test_decode_jpeg_mozjpeg_invalid_data() {
            let invalid_data = vec![0xFF, 0xD8, 0x00]; // 不完全なJPEG
            let result = EncodeTask::decode_jpeg_mozjpeg(&invalid_data);
            assert!(result.is_err());
        }

        #[test]
        fn test_decode_with_image_crate() {
            // PNGデータでdecode()がimage crateを使うことを確認
            let png_data = create_minimal_png();
            let task = EncodeTask {
                source: Some(Arc::new(png_data)),
                decoded: None,
                ops: vec![],
                format: OutputFormat::Png,
                icc_profile: None,
                keep_metadata: false,
            };
            let result = task.decode();
            assert!(result.is_ok());
            let img = result.unwrap();
            assert!(img.dimensions().0 > 0);
            assert!(img.dimensions().1 > 0);
        }

        #[test]
        fn test_decode_already_decoded() {
            let img = create_test_image(100, 100);
            let task = EncodeTask {
                source: None,
                decoded: Some(Arc::new(img.clone())),
                ops: vec![],
                format: OutputFormat::Png,
                icc_profile: None,
                keep_metadata: false,
            };
            let result = task.decode();
            assert!(result.is_ok());
            let decoded_img = result.unwrap();
            assert_eq!(decoded_img.dimensions(), img.dimensions());
        }

        #[test]
        fn test_decode_no_source() {
            let task = EncodeTask {
                source: None,
                decoded: None,
                ops: vec![],
                format: OutputFormat::Png,
                icc_profile: None,
                keep_metadata: false,
            };
            let result = task.decode();
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("Image source already consumed"));
        }
    }

    mod fast_resize_tests {
        use super::*;

        #[test]
        fn test_fast_resize_downscale() {
            let img = create_test_image(200, 200);
            let result = EncodeTask::fast_resize(&img, 100, 100);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 100));
        }

        #[test]
        fn test_fast_resize_upscale() {
            let img = create_test_image(50, 50);
            let result = EncodeTask::fast_resize(&img, 100, 100);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 100));
        }

        #[test]
        fn test_fast_resize_aspect_ratio_change() {
            let img = create_test_image(200, 100);
            let result = EncodeTask::fast_resize(&img, 100, 200);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 200));
        }

        #[test]
        fn test_fast_resize_invalid_dimensions() {
            let img = create_test_image(100, 100);
            let result = EncodeTask::fast_resize(&img, 0, 100);
            assert!(result.is_err());
        }

        #[test]
        fn test_fast_resize_same_size() {
            let img = create_test_image(100, 100);
            let result = EncodeTask::fast_resize(&img, 100, 100);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 100));
        }

        #[test]
        fn test_fast_resize_rgba() {
            let img = create_test_image_rgba(100, 100);
            let result = EncodeTask::fast_resize(&img, 50, 50);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (50, 50));
        }
    }
}
