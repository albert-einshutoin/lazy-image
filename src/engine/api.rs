// src/engine/api.rs
//
// ImageEngine structure and NAPI implementation.
// This is the main public API for the image processing engine.

// BatchResult is used in ts_return_type attribute (line 446) - compiler can't detect this
// Source is used via Source::Memory, Source::Mapped, and Source::Path
#[allow(unused_imports)]
use crate::engine::io::{extract_icc_profile, Source};
#[allow(unused_imports)]
use crate::engine::tasks::{BatchResult, BatchTask, EncodeTask, EncodeWithMetricsTask, WriteFileTask};
use crate::error::LazyImageError;
use crate::ops::{Operation, OutputFormat, PresetConfig};
use image::{DynamicImage, GenericImageView, ImageReader};
use std::io::{BufReader, Cursor};
use std::path::PathBuf;
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
    /// Image source - supports lazy loading from file path
    pub(crate) source: Option<Source>,
    /// Cached raw bytes (loaded on demand for Path sources)
    pub(crate) source_bytes: Option<Arc<Vec<u8>>>,
    /// Decoded image (populated after first decode or on sync operations)
    /// Uses Arc to share decoded image between engines. Combined with Cow<DynamicImage>
    /// in apply_ops, this enables true Copy-on-Write: no deep copy until mutation.
    pub(crate) decoded: Option<Arc<DynamicImage>>,
    /// Queued operations
    pub(crate) ops: Vec<Operation>,
    /// ICC color profile extracted from source image
    pub(crate) icc_profile: Option<Arc<Vec<u8>>>,
    /// Whether to preserve metadata (Exif, ICC, XMP) in output.
    /// Default is false (strip all) for security and smaller file sizes.
    pub(crate) keep_metadata: bool,
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
    /// **ZERO-COPY MEMORY MAPPING**: Uses mmap to map the file into memory.
    /// This enables true zero-copy access - OS pages in only what's needed.
    /// This is the recommended way for server-side processing of large images.
    #[napi(factory, js_name = "fromPath")]
    pub fn from_path(path: String) -> Result<Self> {
        use std::fs::File;
        use memmap2::Mmap;

        let path_buf = PathBuf::from(&path);
        
        // Validate that the file exists (fast check, no read)
        if !path_buf.exists() {
            return Err(napi::Error::from(LazyImageError::file_not_found(path.clone())));
        }

        // Open file and create memory map
        let file = File::open(&path_buf).map_err(|e| {
            napi::Error::from(LazyImageError::file_read_failed(path.clone(), e))
        })?;

        // Safety: We assume the file won't be modified externally during processing.
        // This is a common assumption in image processing libraries.
        // For production use, consider adding file locking (flock) if needed.
        let mmap = unsafe {
            Mmap::map(&file).map_err(|e| {
                napi::Error::from(LazyImageError::file_read_failed(path.clone(), e))
            })?
        };

        let mmap_arc = Arc::new(mmap);
        
        // Extract ICC profile from memory-mapped data
        let icc_profile = extract_icc_profile(mmap_arc.as_ref()).map(Arc::new);

        Ok(ImageEngine {
            source: Some(Source::Mapped(mmap_arc.clone())),
            source_bytes: None, // Not needed for Mapped sources - use as_bytes() directly
            decoded: None,
            ops: Vec::new(),
            icc_profile,
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
    pub fn preset(&mut self, _this: Reference<ImageEngine>, name: String) -> Result<PresetResult> {
        let config = PresetConfig::get(&name)
            .ok_or_else(|| napi::Error::from(LazyImageError::invalid_preset(name)))?;

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
            .map_err(|_e| napi::Error::from(LazyImageError::unsupported_format(format)))?;

        // Use source directly - zero-copy for Memory and Mapped sources
        let source = self.source.clone();
        let decoded = self.decoded.clone();
        let ops = self.ops.clone();
        let icc_profile = self.icc_profile.clone();

        let keep_metadata = self.keep_metadata;

        Ok(AsyncTask::new(EncodeTask {
            source,
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
            .map_err(|_e| napi::Error::from(LazyImageError::unsupported_format(format)))?;

        // Use source directly - zero-copy for Memory and Mapped sources
        let source = self.source.clone();
        let decoded = self.decoded.clone();
        let ops = self.ops.clone();
        let icc_profile = self.icc_profile.clone();
        let keep_metadata = self.keep_metadata;

        Ok(AsyncTask::new(EncodeWithMetricsTask {
            source,
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
            .map_err(|_e| napi::Error::from(LazyImageError::unsupported_format(format)))?;

        // Use source directly - zero-copy for Memory and Mapped sources
        let source = self.source.clone();
        let decoded = self.decoded.clone();
        let ops = self.ops.clone();
        let icc_profile = self.icc_profile.clone();
        let keep_metadata = self.keep_metadata;

        Ok(AsyncTask::new(WriteFileTask {
            source,
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
        // If already decoded, use that
        if let Some(ref img) = self.decoded {
            let (w, h) = img.dimensions();
            return Ok(Dimensions {
                width: w,
                height: h,
            });
        }

        // Try to read dimensions from header only (no full decode)
        let source = self
            .source
            .as_ref()
            .ok_or_else(|| napi::Error::from(LazyImageError::source_consumed()))?;

        // Use as_bytes() to get zero-copy access for Memory and Mapped sources
        if let Some(bytes) = source.as_bytes() {
            // For in-memory or memory-mapped data, use cursor
            let cursor = Cursor::new(bytes);
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
        } else if let Source::Path(path) = source {
            // For file paths, read header directly from file (very fast)
            use std::fs::File;

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
        } else {
            Err(napi::Error::from(LazyImageError::source_consumed()))
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
            .map_err(|_e| napi::Error::from(LazyImageError::unsupported_format(format)))?;
        let ops = self.ops.clone();
        Ok(AsyncTask::new(BatchTask {
            inputs,
            output_dir,
            ops,
            format: output_format,
            concurrency: concurrency.unwrap_or(0), // 0 = use default (CPU cores)
            keep_metadata: self.keep_metadata,
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

// =============================================================================
// INTERNAL IMPLEMENTATION
// =============================================================================

impl ImageEngine {
    /// Get source as a byte slice - zero-copy for Memory and Mapped sources
    /// For Path sources, loads the file first (only when needed)
    #[cfg(feature = "napi")]
    fn ensure_source_slice(&mut self) -> Result<&[u8]> {
        // First, try to get bytes directly (zero-copy for Memory and Mapped)
        // We need to handle Path sources separately to avoid borrow checker issues
        let is_path = matches!(self.source, Some(Source::Path(_)));
        
        if !is_path {
            // For Memory and Mapped sources, get bytes directly
            if let Some(source) = &self.source {
                if let Some(bytes) = source.as_bytes() {
                    // Extract ICC profile if not already extracted
                    if self.icc_profile.is_none() {
                        self.icc_profile = extract_icc_profile(bytes).map(Arc::new);
                    }
                    return Ok(bytes);
                }
            }
            return Err(napi::Error::from(LazyImageError::source_consumed()));
        }

        // For Path sources, we need to load them
        // This should rarely happen as from_path now uses Mapped
        let path = match self.source.take() {
            Some(Source::Path(p)) => p,
            _ => return Err(napi::Error::from(LazyImageError::source_consumed())),
        };

        let data = std::fs::read(&path).map_err(|e| {
            napi::Error::from(LazyImageError::file_read_failed(
                path.to_string_lossy().to_string(),
                e,
            ))
        })?;

        // Extract ICC profile
        if self.icc_profile.is_none() {
            self.icc_profile = extract_icc_profile(&data).map(Arc::new);
        }

        // Convert to Memory source for future use
        // Note: This is a fallback - from_path should use Mapped directly
        let data_arc = Arc::new(data);
        self.source_bytes = Some(data_arc.clone());
        self.source = Some(Source::Memory(data_arc));
        
        // Now return reference from source_bytes
        Ok(self.source_bytes.as_ref().unwrap().as_slice())
    }

    #[cfg(feature = "napi")]
    #[allow(dead_code)]
    fn ensure_decoded(&mut self) -> Result<&DynamicImage> {
        if self.decoded.is_none() {
            // Get source bytes as slice - zero-copy for Memory and Mapped
            let bytes = self.ensure_source_slice()?;

            let img = image::load_from_memory(bytes).map_err(|e| {
                napi::Error::from(LazyImageError::decode_failed(format!(
                    "failed to decode: {e}"
                )))
            })?;

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
            crate::engine::decoder::check_dimensions(w, h)?;

            // Wrap in Arc for sharing (enables Cow::Borrowed in decode())
            self.decoded = Some(Arc::new(img));
        }

        // Safe: we just set it above, use ok_or for safety
        // Return reference to inner DynamicImage
        self.decoded
            .as_ref()
            .map(|arc| arc.as_ref())
            .ok_or_else(||             LazyImageError::internal_panic("decode failed unexpectedly"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::pipeline::fast_resize_owned;
    use image::{DynamicImage, GenericImageView, RgbImage};

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
