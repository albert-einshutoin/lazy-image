// src/engine/api.rs
//
// ImageEngine structure and NAPI implementation.
// This is the main public API for the image processing engine.

// BatchResult is used in ts_return_type attribute (line 446) - compiler can't detect this
// Source is used via Source::Memory and Source::Mapped
use super::firewall::FirewallConfig;
#[cfg(feature = "napi")]
use super::firewall::FirewallPolicy;
#[allow(unused_imports)]
use crate::engine::io::{extract_icc_profile, Source};
#[cfg(feature = "napi")]
#[allow(unused_imports)]
use crate::engine::tasks::{
    BatchResult, BatchTask, EncodeTask, EncodeWithMetricsTask, WriteFileTask,
};
use crate::error::LazyImageError;
#[cfg(not(feature = "napi"))]
use crate::ops::Operation;
#[cfg(feature = "napi")]
use crate::ops::{Operation, OutputFormat, PresetConfig, ResizeFit};
#[cfg(feature = "napi")]
use image::ImageReader;
use image::{DynamicImage, GenericImageView};
#[cfg(feature = "napi")]
use std::io::Cursor;
#[cfg(feature = "napi")]
use std::path::PathBuf;
#[cfg(feature = "napi")]
use std::str::FromStr;
use std::sync::Arc;

#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;

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
    /// Image source - supports in-memory data and memory-mapped files
    pub(crate) source: Option<Source>,
    /// Decoded image (populated after first decode or on sync operations)
    /// Uses Arc to share decoded image between engines. Combined with Cow<DynamicImage>
    /// in apply_ops, this enables true Copy-on-Write: no deep copy until mutation.
    pub(crate) decoded: Option<Arc<DynamicImage>>,
    /// Queued operations
    pub(crate) ops: Vec<Operation>,
    /// ICC color profile extracted from source image
    pub(crate) icc_profile: Option<Arc<Vec<u8>>>,
    /// Whether to auto-apply EXIF Orientation (default: true)
    pub(crate) auto_orient: bool,
    /// Whether to preserve ICC profile in output.
    /// Note: Currently only ICC profile is supported. EXIF and XMP metadata are not preserved.
    /// Default is false (strip all) for security and smaller file sizes.
    pub(crate) keep_metadata: bool,
    pub(crate) firewall: FirewallConfig,
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
        let data_arc = Arc::new(data);

        ImageEngine {
            source: Some(Source::Memory(data_arc)),
            decoded: None,
            ops: Vec::new(),
            icc_profile,
            auto_orient: true,
            keep_metadata: false, // Strip metadata by default for security & smaller files
            firewall: FirewallConfig::disabled(),
        }
    }

    /// Create engine from a file path.
    /// **ZERO-COPY MEMORY MAPPING**: Uses mmap to map the file into memory.
    /// This enables true zero-copy access - OS pages in only what's needed.
    /// This is the recommended way for server-side processing of large images.
    #[napi(factory, js_name = "fromPath")]
    pub fn from_path(env: Env, path: String) -> Result<Self> {
        use memmap2::Mmap;
        use std::fs::File;

        let path_buf = PathBuf::from(&path);

        // Validate that the file exists (fast check, no read)
        if !path_buf.exists() {
            return Err(crate::error::napi_error_with_code(
                &env,
                LazyImageError::file_not_found(path.clone()),
            )?);
        }

        // Open file and create memory map
        let file = match File::open(&path_buf) {
            Ok(file) => file,
            Err(e) => {
                let lazy_err = LazyImageError::file_read_failed(path.clone(), e);
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };

        // Safety: We assume the file won't be modified externally during processing.
        // If modified, decoding may fail, produce corrupted images, or cause OS-dependent SIGBUS/SIGSEGV.
        // For concurrent access concerns, use a copy path or file locking.
        let mmap = unsafe {
            match Mmap::map(&file) {
                Ok(mmap) => mmap,
                Err(e) => {
                    let lazy_err = LazyImageError::mmap_failed(path.clone(), e);
                    return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
                }
            }
        };

        let mmap_arc = Arc::new(mmap);

        // Extract ICC profile from memory-mapped data
        let icc_profile = extract_icc_profile(mmap_arc.as_ref()).map(Arc::new);

        Ok(ImageEngine {
            source: Some(Source::Mapped(mmap_arc)),
            decoded: None,
            ops: Vec::new(),
            icc_profile,
            auto_orient: true,
            keep_metadata: false, // Strip metadata by default for security & smaller files
            firewall: FirewallConfig::disabled(),
        })
    }

    /// Create a clone of this engine (for multi-output scenarios)
    #[napi(js_name = "clone")]
    pub fn clone_engine(&self) -> Result<ImageEngine> {
        Ok(ImageEngine {
            source: self.source.clone(),
            decoded: self.decoded.clone(),
            ops: self.ops.clone(),
            icc_profile: self.icc_profile.clone(),
            auto_orient: self.auto_orient,
            keep_metadata: self.keep_metadata,
            firewall: self.firewall.clone(),
        })
    }

    // =========================================================================
    // PIPELINE OPERATIONS - All return Reference for JS method chaining
    // =========================================================================

    /// Resize image. Width or height can be null to maintain aspect ratio.
    /// When both width and height are provided, `fit` controls behavior:
    /// - "inside" (default): maintain aspect ratio and fit within the box
    /// - "cover": maintain aspect ratio and crop to fill the box
    /// - "fill": ignore aspect ratio and force exact dimensions
    #[napi]
    pub fn resize(
        &mut self,
        this: Reference<ImageEngine>,
        width: Option<u32>,
        height: Option<u32>,
        fit: Option<String>,
    ) -> Result<Reference<ImageEngine>> {
        let fit_mode = if let Some(value) = fit {
            ResizeFit::from_str(&value).map_err(|_| LazyImageError::invalid_resize_fit(value))?
        } else {
            ResizeFit::default()
        };

        self.ops.push(Operation::Resize {
            width,
            height,
            fit: fit_mode,
        });
        Ok(this)
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

    /// Enable or disable EXIF auto-orientation (default: enabled).
    /// `true` = apply EXIF Orientation automatically (sharp-compatible)
    /// `false` = ignore EXIF Orientation
    #[napi(js_name = "autoOrient")]
    pub fn auto_orient(
        &mut self,
        this: Reference<ImageEngine>,
        enabled: bool,
    ) -> Reference<ImageEngine> {
        self.auto_orient = enabled;
        this
    }

    /// Preserve ICC profile in output.
    /// Note: Currently only ICC profile is supported. EXIF and XMP metadata are not preserved.
    /// By default, all metadata is stripped for security (no GPS leak) and smaller file sizes.
    /// Call this method to keep ICC profile when color accuracy is important.
    #[napi(js_name = "keepMetadata")]
    pub fn keep_metadata(&mut self, this: Reference<ImageEngine>) -> Reference<ImageEngine> {
        self.keep_metadata = true;
        this
    }

    /// Enable Image Firewall mode with built-in policies (strict or lenient).
    /// Strict mode enforces aggressive limits and rejects dangerous metadata (best for zero-trust inputs).
    /// Lenient mode keeps generous limits but still guards against decompression bombs.
    #[napi]
    pub fn sanitize(
        &mut self,
        this: Reference<ImageEngine>,
        options: Option<SanitizeOptions>,
    ) -> Result<Reference<ImageEngine>> {
        let requested = options
            .and_then(|opts| opts.policy)
            .unwrap_or_else(|| "strict".to_string());
        let lowered = requested.to_lowercase();
        let policy = match lowered.as_str() {
            "strict" => FirewallPolicy::Strict,
            "lenient" => FirewallPolicy::Lenient,
            _ => return Err(LazyImageError::invalid_firewall_policy(requested).into()),
        };

        self.firewall = FirewallConfig::apply_policy(policy);
        if self.firewall.reject_metadata {
            self.keep_metadata = false;
            self.icc_profile = None;
        }

        if let Some(decoded) = &self.decoded {
            self.firewall
                .enforce_pixels(decoded.width(), decoded.height())?;
        }

        if let Some(source) = &self.source {
            self.firewall.enforce_source_len(source.len())?;
            if let Some(bytes) = source.as_bytes() {
                self.firewall.scan_metadata(bytes)?;
            }
        }

        Ok(this)
    }

    /// Override Image Firewall limits (maxPixels, maxBytes, timeoutMs).
    /// Any field set to 0 disables that particular limit.
    #[napi]
    pub fn limits(
        &mut self,
        this: Reference<ImageEngine>,
        options: FirewallLimitOptions,
    ) -> Result<Reference<ImageEngine>> {
        if !self.firewall.enabled || matches!(self.firewall.policy, FirewallPolicy::Disabled) {
            self.firewall = FirewallConfig::custom();
        } else {
            self.firewall.enabled = true;
            self.firewall.policy = FirewallPolicy::Custom;
        }

        if let Some(max_pixels) = options.max_pixels {
            if max_pixels == 0 {
                self.firewall.max_pixels = None;
            } else {
                self.firewall.max_pixels = Some(max_pixels as u64);
            }
        }

        if let Some(max_bytes) = options.max_bytes {
            if max_bytes == 0 {
                self.firewall.max_bytes = None;
            } else {
                self.firewall.max_bytes = Some(max_bytes as u64);
            }
        }

        if let Some(timeout) = options.timeout_ms {
            if timeout == 0 {
                self.firewall.timeout_ms = None;
            } else {
                self.firewall.timeout_ms = Some(timeout as u64);
            }
        }

        Ok(this)
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
        // Ensure RGB/RGBA pixel format (pixel format normalization, not color space conversion)
        // For true color space conversion with ICC profiles, use a dedicated color management library.
        self.ops.push(Operation::ColorSpace {
            target: crate::ops::ColorSpace::Srgb,
        });
        Ok(this)
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
    pub fn preset(
        &mut self,
        env: Env,
        _this: Reference<ImageEngine>,
        name: String,
    ) -> Result<PresetResult> {
        let config = match PresetConfig::get(&name) {
            Some(config) => config,
            None => {
                let lazy_err = LazyImageError::invalid_preset(name.clone());
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };

        // Apply resize operation
        self.ops.push(Operation::Resize {
            width: config.width,
            height: config.height,
            fit: ResizeFit::Inside,
        });

        // Return preset info for the user to use with toBuffer/toFile
        let (format_str, quality) = match &config.format {
            OutputFormat::Jpeg { quality, .. } => ("jpeg", Some(*quality)),
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
    /// format: "jpeg", "jpg", "png", "webp", "avif"
    /// quality: 1-100 (default: JPEG=85, WebP=80, AVIF=60, ignored for PNG)
    /// fastMode: If true, uses faster encoding for JPEG (2-4x faster, slightly larger files). Default: false.
    ///
    /// **Non-destructive**: This method can be called multiple times on the same engine instance.
    /// The source data is cloned internally, allowing multiple format outputs.
    #[napi(ts_return_type = "Promise<Buffer>")]
    pub fn to_buffer(
        &mut self,
        env: Env,
        format: String,
        quality: Option<u8>,
        fast_mode: Option<bool>,
    ) -> Result<AsyncTask<EncodeTask>> {
        let fast_mode = fast_mode.unwrap_or(false);
        let output_format = match OutputFormat::from_str_with_options(&format, quality, fast_mode) {
            Ok(format) => format,
            Err(_e) => {
                let lazy_err = LazyImageError::unsupported_format(format.clone());
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };

        // Use source directly - zero-copy for Memory and Mapped sources
        let source = self.source.clone();
        let decoded = self.decoded.clone();
        let ops = self.ops.clone();
        let keep_metadata_requested = self.keep_metadata;
        let keep_metadata = keep_metadata_requested && !self.firewall.reject_metadata;
        let auto_orient = self.auto_orient;
        let icc_present = self.icc_profile.is_some();
        let icc_profile = if keep_metadata {
            self.icc_profile.clone()
        } else {
            None
        };

        Ok(AsyncTask::new(EncodeTask {
            source,
            decoded,
            ops,
            format: output_format,
            icc_profile,
            icc_present,
            auto_orient,
            keep_metadata,
            keep_metadata_requested,
            firewall: self.firewall.clone(),
            #[cfg(feature = "napi")]
            last_error: None,
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
        env: Env,
        format: String,
        quality: Option<u8>,
        fast_mode: Option<bool>,
    ) -> Result<AsyncTask<EncodeWithMetricsTask>> {
        let fast_mode = fast_mode.unwrap_or(false);
        let output_format = match OutputFormat::from_str_with_options(&format, quality, fast_mode) {
            Ok(format) => format,
            Err(_e) => {
                let lazy_err = LazyImageError::unsupported_format(format.clone());
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };

        // Use source directly - zero-copy for Memory and Mapped sources
        let source = self.source.clone();
        let decoded = self.decoded.clone();
        let ops = self.ops.clone();
        let keep_metadata_requested = self.keep_metadata;
        let keep_metadata = keep_metadata_requested && !self.firewall.reject_metadata;
        let auto_orient = self.auto_orient;
        let icc_present = self.icc_profile.is_some();
        let icc_profile = if keep_metadata {
            self.icc_profile.clone()
        } else {
            None
        };

        Ok(AsyncTask::new(EncodeWithMetricsTask {
            source,
            decoded,
            ops,
            format: output_format,
            icc_profile,
            icc_present,
            auto_orient,
            keep_metadata,
            keep_metadata_requested,
            firewall: self.firewall.clone(),
            #[cfg(feature = "napi")]
            last_error: None,
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
        env: Env,
        path: String,
        format: String,
        quality: Option<u8>,
        fast_mode: Option<bool>,
    ) -> Result<AsyncTask<WriteFileTask>> {
        let fast_mode = fast_mode.unwrap_or(false);
        let output_format = match OutputFormat::from_str_with_options(&format, quality, fast_mode) {
            Ok(format) => format,
            Err(_e) => {
                let lazy_err = LazyImageError::unsupported_format(format.clone());
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };

        // Use source directly - zero-copy for Memory and Mapped sources
        let source = self.source.clone();
        let decoded = self.decoded.clone();
        let ops = self.ops.clone();
        let keep_metadata_requested = self.keep_metadata;
        let keep_metadata = keep_metadata_requested && !self.firewall.reject_metadata;
        let auto_orient = self.auto_orient;
        let icc_present = self.icc_profile.is_some();
        let icc_profile = if keep_metadata {
            self.icc_profile.clone()
        } else {
            None
        };

        Ok(AsyncTask::new(WriteFileTask {
            source,
            decoded,
            ops,
            format: output_format,
            icc_profile,
            icc_present,
            auto_orient,
            keep_metadata,
            keep_metadata_requested,
            firewall: self.firewall.clone(),
            output_path: path,
            #[cfg(feature = "napi")]
            last_error: None,
        }))
    }

    // =========================================================================
    // SYNC UTILITIES
    // =========================================================================

    /// Get image dimensions WITHOUT full decoding (internal method, no Env required).
    /// For file paths, reads only the header bytes (extremely fast).
    /// For in-memory buffers, uses header-only parsing.
    /// This method is public for testing purposes.
    pub fn dimensions_internal(&mut self) -> std::result::Result<Dimensions, LazyImageError> {
        // If already decoded, use that
        if let Some(ref img) = self.decoded {
            let (w, h) = img.dimensions();
            return Ok(Dimensions {
                width: w,
                height: h,
            });
        }

        // Try to read dimensions from header only (no full decode)
        let source = match self.source.as_ref() {
            Some(source) => source,
            None => {
                return Err(LazyImageError::source_consumed());
            }
        };

        self.firewall.enforce_source_len(source.len())?;

        // Use as_bytes() to get zero-copy access for Memory and Mapped sources
        if let Some(bytes) = source.as_bytes() {
            self.firewall.scan_metadata(bytes)?;
            // For in-memory or memory-mapped data, use cursor
            let cursor = Cursor::new(bytes);
            let reader = match ImageReader::new(cursor).with_guessed_format() {
                Ok(reader) => reader,
                Err(e) => {
                    let err_msg = format!("failed to read image header: {e}");
                    return Err(LazyImageError::decode_failed(err_msg));
                }
            };

            let (width, height) = match reader.into_dimensions() {
                Ok(dims) => dims,
                Err(e) => {
                    let err_msg = format!("failed to read dimensions: {e}");
                    return Err(LazyImageError::decode_failed(err_msg));
                }
            };

            Ok(Dimensions { width, height })
        } else {
            Err(LazyImageError::source_consumed())
        }
    }

    /// Get image dimensions WITHOUT full decoding.
    /// For file paths, reads only the header bytes (extremely fast).
    /// For in-memory buffers, uses header-only parsing.
    #[napi]
    pub fn dimensions(&mut self, env: Env) -> Result<Dimensions> {
        match self.dimensions_internal() {
            Ok(dims) => Ok(dims),
            Err(lazy_err) => {
                let napi_err = crate::error::napi_error_with_code(&env, lazy_err)?;
                Err(napi_err)
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
    /// - fastMode: Optional fast mode flag (only applies to JPEG, default: false)
    /// - concurrency: Optional number of parallel workers:
    ///   - 0 or undefined: Auto-detect based on CPU cores and memory limits (smart concurrency)
    ///     Detects container memory limits (cgroup v1/v2) and adjusts to prevent OOM kills.
    ///     Ideal for serverless/containerized environments with memory constraints.
    ///   - 1-1024: Manual override - use specified number of concurrent operations
    #[napi(js_name = "processBatch", ts_return_type = "Promise<BatchResult[]>")]
    pub fn process_batch(
        &self,
        env: Env,
        inputs: Vec<String>,
        output_dir: String,
        format: String,
        quality: Option<u8>,
        fast_mode: Option<bool>,
        concurrency: Option<u32>,
    ) -> Result<AsyncTask<BatchTask>> {
        let fast_mode = fast_mode.unwrap_or(false);
        let output_format = match OutputFormat::from_str_with_options(&format, quality, fast_mode) {
            Ok(format) => format,
            Err(_e) => {
                let lazy_err = LazyImageError::unsupported_format(format.clone());
                return Err(crate::error::napi_error_with_code(&env, lazy_err)?);
            }
        };
        let ops = self.ops.clone();
        Ok(AsyncTask::new(BatchTask {
            inputs,
            output_dir,
            ops,
            format: output_format,
            concurrency: concurrency.unwrap_or(0), // 0 = use default (CPU cores)
            keep_metadata: self.keep_metadata && !self.firewall.reject_metadata,
            auto_orient: self.auto_orient,
            firewall: self.firewall.clone(),
            #[cfg(feature = "napi")]
            last_error: None,
        }))
    }
}

#[cfg(feature = "napi")]
#[napi(object)]
pub struct SanitizeOptions {
    pub policy: Option<String>,
}

#[cfg(feature = "napi")]
#[napi(object)]
pub struct FirewallLimitOptions {
    pub max_pixels: Option<u32>,
    pub max_bytes: Option<u32>,
    pub timeout_ms: Option<u32>,
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

// =============================================================================
// INTERNAL IMPLEMENTATION
// =============================================================================

impl ImageEngine {
    /// Get source as a byte slice - zero-copy for Memory and Mapped sources
    #[cfg(feature = "napi")]
    fn ensure_source_slice(&mut self) -> Result<&[u8]> {
        // Get bytes directly (zero-copy for Memory and Mapped)
        if let Some(source) = &self.source {
            if let Some(bytes) = source.as_bytes() {
                // Extract ICC profile if not already extracted
                if self.icc_profile.is_none() {
                    self.icc_profile = extract_icc_profile(bytes).map(Arc::new);
                }
                return Ok(bytes);
            }
        }
        Err(napi::Error::from(LazyImageError::source_consumed()))
    }

    #[cfg(feature = "napi")]
    #[allow(dead_code)]
    fn ensure_decoded(&mut self) -> Result<&DynamicImage> {
        if self.decoded.is_none() {
            // Get source bytes as slice - zero-copy for Memory and Mapped
            let bytes = self.ensure_source_slice()?;

            crate::engine::decoder::ensure_dimensions_safe(bytes)?;

            let (img, _detected_format) =
                crate::engine::decoder::decode_image(bytes).map_err(napi::Error::from)?;

            // Security check: reject decompression bombs
            let (w, h) = img.dimensions();
            crate::engine::decoder::check_dimensions(w, h)?;

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

    /// Get source as a byte slice - zero-copy for Memory and Mapped sources
    #[cfg(not(feature = "napi"))]
    #[allow(dead_code)]
    fn ensure_source_slice(&mut self) -> std::result::Result<&[u8], LazyImageError> {
        // Get bytes directly (zero-copy for Memory and Mapped)
        if let Some(source) = &self.source {
            if let Some(bytes) = source.as_bytes() {
                // Extract ICC profile if not already extracted
                if self.icc_profile.is_none() {
                    self.icc_profile = extract_icc_profile(bytes).map(Arc::new);
                }
                return Ok(bytes);
            }
        }
        Err(LazyImageError::source_consumed())
    }

    #[cfg(not(feature = "napi"))]
    #[allow(dead_code)]
    fn ensure_decoded(&mut self) -> std::result::Result<&DynamicImage, LazyImageError> {
        if self.decoded.is_none() {
            // Get source bytes as slice - zero-copy for Memory and Mapped
            let bytes = self.ensure_source_slice()?;

            crate::engine::decoder::ensure_dimensions_safe(bytes)?;

            let (img, _detected_format) = crate::engine::decoder::decode_image(bytes)?;

            // Security check: reject decompression bombs
            let (w, h) = img.dimensions();
            crate::engine::decoder::check_dimensions(w, h)?;

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

#[cfg(test)]
mod tests {
    use crate::engine::pipeline::fast_resize_owned;
    #[allow(unused_imports)]
    use image::GenericImageView;
    use image::{DynamicImage, RgbImage};

    fn create_test_image(width: u32, height: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        }))
    }

    #[test]
    fn fast_resize_owned_returns_error_instead_of_dummy_image() {
        let img = create_test_image(1, 1);
        let err = fast_resize_owned(img, 0, 10).expect_err("expected resize failure");
        assert_eq!(err.source_dims, (1, 1));
        assert_eq!(err.target_dims, (0, 10));
        assert!(err.reason.contains("invalid dimensions"));
    }
}
