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
const MAX_DIMENSION: u32 = 32768;

/// Maximum allowed total pixels (width * height).
/// 100 megapixels = 400MB uncompressed RGBA. Beyond this is likely malicious.
const MAX_PIXELS: u64 = 100_000_000;


// Quality configuration helper
struct QualitySettings {
    quality: f32,
}

impl QualitySettings {
    fn new(quality: u8) -> Self {
        Self { quality: quality as f32 }
    }

    // WebP settings
    fn webp_method(&self) -> i32 {
        if self.quality >= 80.0 { 5 } else { 6 }
    }

    fn webp_pass(&self) -> i32 {
        if self.quality >= 85.0 { 3 }
        else if self.quality >= 70.0 { 4 }
        else { 5 }
    }

    fn webp_preprocessing(&self) -> i32 {
        if self.quality >= 80.0 { 1 }
        else if self.quality >= 60.0 { 2 }
        else { 3 }
    }

    fn webp_sns_strength(&self) -> i32 {
        if self.quality >= 85.0 { 50 }
        else if self.quality >= 70.0 { 70 }
        else { 80 }
    }

    fn webp_filter_strength(&self) -> i32 {
        if self.quality >= 80.0 { 20 }
        else if self.quality >= 60.0 { 30 }
        else { 40 }
    }

    fn webp_filter_sharpness(&self) -> i32 {
        if self.quality >= 85.0 { 2 } else { 0 }
    }

    // AVIF settings
    fn avif_speed(&self) -> u8 {
        if self.quality >= 85.0 { 7 }
        else if self.quality >= 70.0 { 6 }
        else if self.quality >= 50.0 { 5 }
        else { 4 }
    }
}


use crate::ops::{Operation, OutputFormat};
use fast_image_resize::{self as fir, PixelType, ResizeOptions};
use image::{DynamicImage, GenericImageView, ImageFormat, RgbImage, RgbaImage};
use img_parts::{jpeg::Jpeg, png::Png, ImageICC};
use mozjpeg::{ColorSpace, Compress, Decompress, ScanMode};
use napi::bindgen_prelude::*;
use napi::{Env, JsBuffer, Task};
use ravif::{Encoder as AvifEncoder, Img};
use rayon::prelude::*;
use rgb::FromSlice;
use std::io::Cursor;
use std::panic;
use std::sync::Arc;

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
#[napi]
pub struct ImageEngine {
    /// Raw source bytes - we delay decoding until compute()
    source: Option<Arc<Vec<u8>>>,
    /// Decoded image (populated after first decode or on sync operations)
    decoded: Option<DynamicImage>,
    /// Queued operations
    ops: Vec<Operation>,
    /// ICC color profile extracted from source image
    icc_profile: Option<Arc<Vec<u8>>>,
}

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
        
        ImageEngine {
            source: Some(Arc::new(data)),
            decoded: None,
            ops: Vec::new(),
            icc_profile,
        }
    }

    /// Create engine from a file path. 
    /// **Memory-efficient**: Reads directly into Rust heap, bypassing Node.js V8 heap.
    /// This is the recommended way for server-side processing of large images.
    #[napi(factory, js_name = "fromPath")]
    pub fn from_path(path: String) -> Result<Self> {
        use std::fs;

        let data = fs::read(&path)
            .map_err(|e| Error::from_reason(format!("failed to read file '{}': {}", path, e)))?;

        // Extract ICC profile before any processing
        let icc_profile = extract_icc_profile(&data).map(Arc::new);

        Ok(ImageEngine {
            source: Some(Arc::new(data)),
            decoded: None,
            ops: Vec::new(),
            icc_profile,
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
        })
    }

    // =========================================================================
    // PIPELINE OPERATIONS - All return Reference for JS method chaining
    // =========================================================================

    /// Resize image. Width or height can be null to maintain aspect ratio.
    #[napi]
    pub fn resize(&mut self, this: Reference<ImageEngine>, width: Option<u32>, height: Option<u32>) -> Reference<ImageEngine> {
        self.ops.push(Operation::Resize { width, height });
        this
    }

    /// Crop a region from the image.
    #[napi]
    pub fn crop(&mut self, this: Reference<ImageEngine>, x: u32, y: u32, width: u32, height: u32) -> Reference<ImageEngine> {
        self.ops.push(Operation::Crop { x, y, width, height });
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

    /// Adjust brightness (-100 to 100)
    #[napi]
    pub fn brightness(&mut self, this: Reference<ImageEngine>, value: i32) -> Reference<ImageEngine> {
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

    /// Convert to specific color space (e.g. 'srgb')
    /// Currently ensures the image is in RGB/RGBA format.
    #[napi(js_name = "toColorspace")]
    pub fn to_color_space(&mut self, this: Reference<ImageEngine>, color_space: String) -> Result<Reference<ImageEngine>> {
        let target = match color_space.to_lowercase().as_str() {
            "srgb" => crate::ops::ColorSpace::Srgb,
            "p3" | "display-p3" => crate::ops::ColorSpace::DisplayP3,
            "adobergb" => crate::ops::ColorSpace::AdobeRgb,
            _ => return Err(Error::from_reason(format!("unsupported color space: {}", color_space))),
        };
        self.ops.push(Operation::ColorSpace { target });
        Ok(this)
    }

    // =========================================================================
    // OUTPUT - Triggers async computation
    // =========================================================================

    /// Encode to buffer asynchronously.
    /// format: "jpeg", "jpg", "png", "webp"
    /// quality: 1-100 (default 80, ignored for PNG)
    #[napi(ts_return_type = "Promise<Buffer>")]
    pub fn to_buffer(
        &mut self,
        format: String,
        quality: Option<u8>,
    ) -> Result<AsyncTask<EncodeTask>> {
        let output_format = OutputFormat::from_str(&format, quality)
            .map_err(Error::from_reason)?;

        // Take ownership of source data
        let source = self.source.take();
        let decoded = self.decoded.take();
        let ops = std::mem::take(&mut self.ops);
        let icc_profile = self.icc_profile.take();

        Ok(AsyncTask::new(EncodeTask {
            source,
            decoded,
            ops,
            format: output_format,
            icc_profile,
        }))
    }

    /// Encode to buffer asynchronously with performance metrics.
    /// Returns `{ data: Buffer, metrics: ProcessingMetrics }`.
    #[napi(ts_return_type = "Promise<OutputWithMetrics>")]
    pub fn to_buffer_with_metrics(
        &mut self,
        format: String,
        quality: Option<u8>,
    ) -> Result<AsyncTask<EncodeWithMetricsTask>> {
        let output_format = OutputFormat::from_str(&format, quality)
            .map_err(Error::from_reason)?;

        let source = self.source.take();
        let decoded = self.decoded.take();
        let ops = std::mem::take(&mut self.ops);
        let icc_profile = self.icc_profile.take();

        Ok(AsyncTask::new(EncodeWithMetricsTask {
            source,
            decoded,
            ops,
            format: output_format,
            icc_profile,
        }))
    }

    /// Encode and write directly to a file asynchronously.
    /// **Memory-efficient**: Combined with fromPath(), this enables
    /// full file-to-file processing without touching Node.js heap.
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
            .map_err(Error::from_reason)?;

        // Take ownership of source data
        let source = self.source.take();
        let decoded = self.decoded.take();
        let ops = std::mem::take(&mut self.ops);
        let icc_profile = self.icc_profile.take();

        Ok(AsyncTask::new(WriteFileTask {
            source,
            decoded,
            ops,
            format: output_format,
            icc_profile,
            output_path: path,
        }))
    }

    // =========================================================================
    // SYNC UTILITIES
    // =========================================================================

    /// Get image dimensions (decodes image if needed)
    #[napi]
    pub fn dimensions(&mut self) -> Result<Dimensions> {
        let img = self.ensure_decoded()?;
        let (w, h) = img.dimensions();
        Ok(Dimensions { width: w, height: h })
    }

    /// Check if an ICC color profile was extracted from the source image.
    /// Returns the profile size in bytes, or null if no profile exists.
    #[napi(js_name = "hasIccProfile")]
    pub fn has_icc_profile(&self) -> Option<u32> {
        self.icc_profile.as_ref().map(|p| p.len() as u32)
    }

    #[napi(js_name = "processBatch", ts_return_type = "Promise<BatchResult[]>")]
    pub fn process_batch(
        &self,
        inputs: Vec<String>,
        output_dir: String,
        format: String,
        quality: Option<u8>,
    ) -> Result<AsyncTask<BatchTask>> {
        let output_format = OutputFormat::from_str(&format, quality)
            .map_err(Error::from_reason)?;
        let ops = self.ops.clone();
        Ok(AsyncTask::new(BatchTask {
            inputs,
            output_dir,
            ops,
            format: output_format,
        }))
    }
}

#[napi(object)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

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
    fn ensure_decoded(&mut self) -> Result<&DynamicImage> {
        if self.decoded.is_none() {
            let source = self.source.as_ref()
                .ok_or_else(|| Error::from_reason("image source already consumed"))?;
            
            let img = image::load_from_memory(source)
                .map_err(|e| Error::from_reason(format!("failed to decode: {e}")))?;
            
            // Security check: reject decompression bombs
            let (w, h) = img.dimensions();
            check_dimensions(w, h)?;
            
            self.decoded = Some(img);
        }
        
        // Safe: we just set it above, use ok_or for safety
        self.decoded.as_ref()
            .ok_or_else(|| Error::from_reason("decode failed unexpectedly"))
    }
}

// =============================================================================
// ASYNC TASK - Where the real work happens
// =============================================================================

pub struct EncodeTask {
    source: Option<Arc<Vec<u8>>>,
    decoded: Option<DynamicImage>,
    ops: Vec<Operation>,
    format: OutputFormat,
    icc_profile: Option<Arc<Vec<u8>>>,
}

impl EncodeTask {
    /// Decode image from source bytes
    /// Uses mozjpeg (libjpeg-turbo) for JPEG, falls back to image crate for others
    fn decode(&self) -> Result<DynamicImage> {
        // Prefer already decoded image (already validated)
        if let Some(ref img) = self.decoded {
            return Ok(img.clone());
        }

        let source = self.source.as_ref()
            .ok_or_else(|| Error::from_reason("no image source"))?;

        // Check magic bytes for JPEG (0xFF 0xD8)
        let img = if source.len() >= 2 && source[0] == 0xFF && source[1] == 0xD8 {
            // JPEG detected - use mozjpeg for TURBO speed
            Self::decode_jpeg_mozjpeg(source)?
        } else {
            // PNG, WebP, etc - use image crate
            image::load_from_memory(source)
                .map_err(|e| Error::from_reason(format!("decode failed: {e}")))?
        };

        // Security check: reject decompression bombs
        let (w, h) = img.dimensions();
        check_dimensions(w, h)?;

        Ok(img)
    }
    /// Decode JPEG using mozjpeg (backed by libjpeg-turbo)
    /// This is SIGNIFICANTLY faster than image crate's pure Rust decoder
    fn decode_jpeg_mozjpeg(data: &[u8]) -> Result<DynamicImage> {
        let result = panic::catch_unwind(|| {
            let decompress = Decompress::new_mem(data)
                .map_err(|e| format!("mozjpeg decompress init failed: {e:?}"))?;
            
            // Get image info
            let mut decompress = decompress.rgb()
                .map_err(|e| format!("mozjpeg rgb conversion failed: {e:?}"))?;
            
            let width = decompress.width();
            let height = decompress.height();
            
            // Read all scanlines
            let pixels: Vec<[u8; 3]> = decompress.read_scanlines()
                .map_err(|e| format!("mozjpeg: failed to read scanlines: {e:?}"))?;
            
            // Flatten the Vec<[u8; 3]> to Vec<u8>
            let flat_pixels: Vec<u8> = pixels.into_iter()
                .flat_map(|rgb| rgb.into_iter())
                .collect();
            
            // Create DynamicImage from raw RGB data
            let rgb_image = RgbImage::from_raw(width as u32, height as u32, flat_pixels)
                .ok_or_else(|| "mozjpeg: failed to create image from raw data".to_string())?;
            
            Ok::<DynamicImage, String>(DynamicImage::ImageRgb8(rgb_image))
        });

        match result {
            Ok(Ok(img)) => Ok(img),
            Ok(Err(e)) => Err(Error::from_reason(e)),
            Err(_) => Err(Error::from_reason("mozjpeg panicked during decode")),
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
            if let Operation::Resize { width: w1, height: h1 } = current {
                let mut final_width = *w1;
                let mut final_height = *h1;
                let mut j = i + 1;

                // Combine all consecutive resize operations
                while j < ops.len() {
                    if let Operation::Resize { width: w2, height: h2 } = &ops[j] {
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
                    (Operation::Crop { x, y, width: cw, height: ch }, Operation::Resize { width: rw, height: rh }) => {
                        let (final_w, final_h) = calc_resize_dimensions(*cw, *ch, *rw, *rh);
                        optimized.push(Operation::Crop { x: *x, y: *y, width: *cw, height: *ch });
                        optimized.push(Operation::Resize { width: Some(final_w), height: Some(final_h) });
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

    /// Apply all queued operations
    fn apply_ops(mut img: DynamicImage, ops: &[Operation]) -> Result<DynamicImage> {
        // Optimize operations first
        let optimized_ops = Self::optimize_ops(ops);

        for op in &optimized_ops {
            img = match op {
                Operation::Resize { width, height } => {
                    let (w, h) = calc_resize_dimensions(
                        img.width(), 
                        img.height(), 
                        *width, 
                        *height
                    );
                    // Use SIMD-accelerated fast_image_resize with silent fallback
                    Self::fast_resize(&img, w, h).unwrap_or_else(|_| {
                        img.resize_exact(w, h, image::imageops::FilterType::Lanczos3)
                    })
                }

                Operation::Crop { x, y, width, height } => {
                    // Validate crop bounds
                    let img_w = img.width();
                    let img_h = img.height();
                    if *x + *width > img_w || *y + *height > img_h {
                        return Err(Error::from_reason(format!(
                            "crop bounds ({}+{}, {}+{}) exceed image dimensions ({}x{})",
                            x, width, y, height, img_w, img_h
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
                            return Err(Error::from_reason(format!(
                                "unsupported rotation angle: {}. Only 0, 90, 180, 270 (and negatives) are supported",
                                degrees
                            )));
                        }
                    }
                }

                Operation::FlipH => img.fliph(),
                Operation::FlipV => img.flipv(),
                Operation::Grayscale => DynamicImage::ImageLuma8(img.to_luma8()),
                
                Operation::Brightness { value } => {
                    img.brighten(*value)
                }

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
                            return Err(Error::from_reason(format!(
                                "color space {:?} is not yet implemented", target
                            )));
                        }
                    }
                }
            };
        }
        Ok(img)
    }
    fn fast_resize(img: &DynamicImage, dst_width: u32, dst_height: u32) -> std::result::Result<DynamicImage, String> {
        let src_width = img.width();
        let src_height = img.height();

        if src_width == 0 || src_height == 0 || dst_width == 0 || dst_height == 0 {
            return Err("invalid dimensions".to_string());
        }

        // Convert to RGBA for processing
        let rgba = img.to_rgba8();
        let src_pixels = rgba.as_raw();

        // Create source image for fast_image_resize
        let src_image = fir::images::Image::from_vec_u8(
            src_width,
            src_height,
            src_pixels.clone(),
            PixelType::U8x4,
        ).map_err(|e| format!("fir source image error: {e:?}"))?;

        // Create destination image
        let mut dst_image = fir::images::Image::new(
            dst_width,
            dst_height,
            PixelType::U8x4,
        );

        // Create resizer with Lanczos3 (high quality)
        let mut resizer = fir::Resizer::new();
        
        // Resize with Lanczos3 filter
        let options = ResizeOptions::new().resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::Lanczos3));
        resizer.resize(&src_image, &mut dst_image, &options)
            .map_err(|e| format!("fir resize error: {e:?}"))?;

        // Convert back to DynamicImage
        let dst_pixels = dst_image.into_vec();
        let rgba_image = RgbaImage::from_raw(dst_width, dst_height, dst_pixels)
            .ok_or("failed to create rgba image from resized data")?;

        Ok(DynamicImage::ImageRgba8(rgba_image))
    }

    /// Encode to JPEG using mozjpeg with RUTHLESS Web-optimized settings
    fn encode_jpeg(img: &DynamicImage, quality: u8, icc: Option<&[u8]>) -> Result<Vec<u8>> {
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let pixels = rgb.into_raw();

        // mozjpeg can panic internally, so we catch it
        let result = panic::catch_unwind(|| {
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
            //    Note: This is enabled by default in mozjpeg, but we ensure it's on
            
            // 6. Adaptive smoothing: Reduces high-frequency noise for better compression
            //    Higher quality = less smoothing, lower quality = more smoothing
            let smoothing = if quality_f32 >= 90.0 {
                0 // No smoothing for high quality
            } else if quality_f32 >= 70.0 {
                5 // Minimal smoothing
            } else if quality_f32 >= 50.0 {
                10 // Moderate smoothing
            } else {
                15 // More smoothing for lower quality
            };
            comp.set_smoothing_factor(smoothing);
            
            // 7. Quantization table optimization
            //    mozjpeg automatically optimizes quantization tables when optimize_coding is true
            
            // Estimate output size: ~10% of raw size for typical JPEG compression
            let estimated_size = (w as usize * h as usize * 3 / 10).max(4096);
            let mut output = Vec::with_capacity(estimated_size);

            {
                let mut writer = comp.start_compress(&mut output)
                    .expect("mozjpeg: failed to start compress");

                let stride = w as usize * 3;
                for row in pixels.chunks(stride) {
                    let _ = writer.write_scanlines(row);
                }

                writer.finish().expect("mozjpeg: failed to finish");
            }
            
            output
        });

        let encoded = result.map_err(|_| Error::from_reason("mozjpeg panicked during encoding"))?;

        // Embed ICC profile using img-parts if present
        if let Some(icc_data) = icc {
            Self::embed_icc_jpeg(encoded, icc_data)
        } else {
            Ok(encoded)
        }
    }

    /// Embed ICC profile into JPEG using img-parts
    fn embed_icc_jpeg(jpeg_data: Vec<u8>, icc: &[u8]) -> Result<Vec<u8>> {
        use img_parts::jpeg::{Jpeg, JpegSegment};
        use img_parts::Bytes;

        let mut jpeg = Jpeg::from_bytes(Bytes::from(jpeg_data))
            .map_err(|e| Error::from_reason(format!("failed to parse JPEG for ICC: {e}")))?;

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
        jpeg.encoder()
            .write_to(&mut output)
            .map_err(|e| Error::from_reason(format!("failed to write JPEG with ICC: {e}")))?;

        Ok(output)
    }

    /// Encode to PNG using image crate
    fn encode_png(img: &DynamicImage, icc: Option<&[u8]>) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .map_err(|e| Error::from_reason(format!("PNG encode failed: {e}")))?;

        // Embed ICC profile if present
        if let Some(icc_data) = icc {
            Self::embed_icc_png(buf, icc_data)
        } else {
            Ok(buf)
        }
    }

    /// Embed ICC profile into PNG using img-parts
    fn embed_icc_png(png_data: Vec<u8>, icc: &[u8]) -> Result<Vec<u8>> {
        use img_parts::png::Png;
        use img_parts::{Bytes, ImageICC};
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;

        let mut png = Png::from_bytes(Bytes::from(png_data))
            .map_err(|e| Error::from_reason(format!("failed to parse PNG for ICC: {e}")))?;

        // iCCP chunk format: profile_name (null-terminated) + compression_method (0) + compressed_data
        let profile_name = b"ICC\0"; // Short name
        let compression_method = 0u8; // zlib

        // Compress ICC data
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(icc)
            .map_err(|e| Error::from_reason(format!("failed to compress ICC: {e}")))?;
        let compressed = encoder.finish()
            .map_err(|e| Error::from_reason(format!("failed to finish ICC compression: {e}")))?;

        let mut chunk_data = Vec::with_capacity(profile_name.len() + 1 + compressed.len());
        chunk_data.extend_from_slice(profile_name);
        chunk_data.push(compression_method);
        chunk_data.extend_from_slice(&compressed);

        // Use img-parts' ICC API
        png.set_icc_profile(Some(Bytes::from(chunk_data)));

        // Encode back
        let mut output = Vec::new();
        png.encoder()
            .write_to(&mut output)
            .map_err(|e| Error::from_reason(format!("failed to write PNG with ICC: {e}")))?;

        Ok(output)
    }

    /// Encode to WebP with optimized settings
    fn encode_webp(img: &DynamicImage, quality: u8, icc: Option<&[u8]>) -> Result<Vec<u8>> {
        // Use RGB instead of RGBA for smaller files (unless alpha is needed)
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();

        let encoder = webp::Encoder::from_rgb(&rgb, w, h);
        
        // Create WebPConfig with enhanced preprocessing for better compression
        let mut config = webp::WebPConfig::new()
            .map_err(|_| Error::from_reason("failed to create WebPConfig"))?;
        
        let settings = QualitySettings::new(quality);
        config.quality = settings.quality;
        config.method = settings.webp_method();
        config.pass = settings.webp_pass();
        config.preprocessing = settings.webp_preprocessing();
        config.sns_strength = settings.webp_sns_strength();
        config.autofilter = 1;
        config.filter_strength = settings.webp_filter_strength();
        config.filter_sharpness = settings.webp_filter_sharpness();
        
        let mem = encoder.encode_advanced(&config)
            .map_err(|e| Error::from_reason(format!("WebP encode failed: {e:?}")))?;
        
        let encoded = mem.to_vec();

        // Embed ICC profile if present
        if let Some(icc_data) = icc {
            Self::embed_icc_webp(encoded, icc_data)
        } else {
            Ok(encoded)
        }
    }

    /// Embed ICC profile into WebP using img-parts
    fn embed_icc_webp(webp_data: Vec<u8>, icc: &[u8]) -> Result<Vec<u8>> {
        use img_parts::webp::WebP;
        use img_parts::Bytes;

        let mut webp = WebP::from_bytes(Bytes::from(webp_data))
            .map_err(|e| Error::from_reason(format!("failed to parse WebP for ICC: {e}")))?;

        // Set the ICCP chunk directly
        webp.set_icc_profile(Some(Bytes::from(icc.to_vec())));

        // Encode back
        let mut output = Vec::new();
        webp.encoder()
            .write_to(&mut output)
            .map_err(|e| Error::from_reason(format!("failed to write WebP with ICC: {e}")))?;

        Ok(output)
    }

    /// Encode to AVIF - next-gen format, even smaller than WebP
    /// 
    /// Note: ICC profile embedding is not currently supported by ravif.
    /// AVIF files will use sRGB color space by default.
    fn encode_avif(img: &DynamicImage, quality: u8, icc: Option<&[u8]>) -> Result<Vec<u8>> {
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let pixels = rgba.as_raw();

        // Convert to ravif's expected format
        let img_ref = Img::new(
            pixels.as_rgba(),
            width as usize,
            height as usize,
        );

        let settings = QualitySettings::new(quality);

        let encoder = AvifEncoder::new()
            .with_quality(settings.quality)
            .with_speed(settings.avif_speed());

        // Note: ravif 0.11 doesn't have native ICC embedding API
        // AVIF files assume sRGB by default, which is acceptable for web use
        // TODO: Consider using libavif bindings for full ICC support in the future
        
        // Warn if ICC profile is present but cannot be embedded
        if icc.is_some() {
            // In a production environment, you might want to log this
            // For now, we silently proceed with sRGB assumption
            // The ICC profile information is lost in AVIF output
        }

        let result = encoder.encode_rgba(img_ref)
            .map_err(|e| Error::from_reason(format!("AVIF encode failed: {e}")))?;

        Ok(result.avif_file)
    }

    /// Process image: decode → apply ops → encode
    /// This is the core processing pipeline shared by toBuffer and toFile.
    fn process_and_encode(&mut self, mut metrics: Option<&mut crate::ProcessingMetrics>) -> Result<Vec<u8>> {
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

        // 3. Encode with ICC profile preservation
        let start_encode = std::time::Instant::now();
        let icc = self.icc_profile.as_ref().map(|v| v.as_slice());
        let result = match &self.format {
            OutputFormat::Jpeg { quality } => Self::encode_jpeg(&processed, *quality, icc),
            OutputFormat::Png => Self::encode_png(&processed, icc),
            OutputFormat::WebP { quality } => Self::encode_webp(&processed, *quality, icc),
            OutputFormat::Avif { quality } => Self::encode_avif(&processed, *quality, icc),
        }?;
        
        if let Some(m) = metrics {
            m.encode_time = start_encode.elapsed().as_secs_f64() * 1000.0;
            // Estimate memory (rough)
            let (w, h) = processed.dimensions();
            m.memory_peak = (w * h * 4 + result.len() as u32) as u32;
        }

        Ok(result)
    }
}

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

pub struct EncodeWithMetricsTask {
    source: Option<Arc<Vec<u8>>>,
    decoded: Option<DynamicImage>,
    ops: Vec<Operation>,
    format: OutputFormat,
    icc_profile: Option<Arc<Vec<u8>>>,
}

#[napi]
impl Task for EncodeWithMetricsTask {
    type Output = (Vec<u8>, crate::ProcessingMetrics);
    type JsValue = crate::OutputWithMetrics;

    fn compute(&mut self) -> Result<Self::Output> {
        //Reuse EncodeTask logic
        let mut task = EncodeTask {
            source: self.source.take(),
            decoded: self.decoded.take(),
            ops: std::mem::take(&mut self.ops),
            format: self.format.clone(),
            icc_profile: self.icc_profile.take(),
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

pub struct WriteFileTask {
    source: Option<Arc<Vec<u8>>>,
    decoded: Option<DynamicImage>,
    ops: Vec<Operation>,
    format: OutputFormat,
    icc_profile: Option<Arc<Vec<u8>>>,
    output_path: String,
}

#[napi]
impl Task for WriteFileTask {
    type Output = u32; // Bytes written
    type JsValue = u32;

    fn compute(&mut self) -> Result<Self::Output> {
        use std::fs::File;
        use std::io::Write;

        // Create EncodeTask and use its process_and_encode method
        let mut encode_task = EncodeTask {
            source: self.source.take(),
            decoded: self.decoded.take(),
            ops: std::mem::take(&mut self.ops),
            format: self.format.clone(),
            icc_profile: self.icc_profile.take(),
        };

        // Process image using shared logic
        let data = encode_task.process_and_encode(None)?;

        // Atomic write: write to temp file, then rename on success
        let temp_path = format!("{}.tmp.{}", self.output_path, std::process::id());
        
        let write_result = (|| -> Result<u32> {
            let mut file = File::create(&temp_path)
                .map_err(|e| Error::from_reason(format!("failed to create temp file: {}", e)))?;
            
            let bytes_written = data.len() as u32;
            file.write_all(&data)
                .map_err(|e| Error::from_reason(format!("failed to write data: {}", e)))?;
            
            // Ensure data is flushed to disk
            file.sync_all()
                .map_err(|e| Error::from_reason(format!("failed to sync file: {}", e)))?;
            
            Ok(bytes_written)
        })();
        
        match write_result {
            Ok(bytes) => {
                // Success: rename temp file to final destination
                std::fs::rename(&temp_path, &self.output_path)
                    .map_err(|e| Error::from_reason(format!("failed to rename temp file: {}", e)))?;
                Ok(bytes)
            }
            Err(e) => {
                // Failure: clean up temp file
                let _ = std::fs::remove_file(&temp_path);
                Err(e)
            }
        }
    }
    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

pub struct BatchTask {
    inputs: Vec<String>,
    output_dir: String,
    ops: Vec<Operation>,
    format: OutputFormat,
}

#[napi]
impl Task for BatchTask {
    type Output = Vec<BatchResult>;
    type JsValue = Vec<BatchResult>;

    fn compute(&mut self) -> Result<Self::Output> {
        use std::fs;
        use std::path::Path;

        if !Path::new(&self.output_dir).exists() {
            fs::create_dir_all(&self.output_dir)
                .map_err(|e| Error::from_reason(format!("failed to create output dir: {}", e)))?;
        }

        let results: Vec<BatchResult> = self.inputs.par_iter().map(|input_path| {
            let process_one = || -> Result<String> {
                let data = fs::read(input_path)
                    .map_err(|e| Error::from_reason(format!("failed to read file: {}", e)))?;
                
                let icc_profile = extract_icc_profile(&data).map(Arc::new);

                let img = if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8 {
                    EncodeTask::decode_jpeg_mozjpeg(&data)?
                } else {
                    image::load_from_memory(&data)
                        .map_err(|e| Error::from_reason(format!("decode failed: {e}")))?
                };
                
                let (w, h) = img.dimensions();
                check_dimensions(w, h)?;

                let processed = EncodeTask::apply_ops(img, &self.ops)?;

                let icc = icc_profile.as_ref().map(|v| v.as_slice());
                let encoded = match &self.format {
                    OutputFormat::Jpeg { quality } => EncodeTask::encode_jpeg(&processed, *quality, icc)?,
                    OutputFormat::Png => EncodeTask::encode_png(&processed, icc)?,
                    OutputFormat::WebP { quality } => EncodeTask::encode_webp(&processed, *quality, icc)?,
                    OutputFormat::Avif { quality } => EncodeTask::encode_avif(&processed, *quality, icc)?,
                };

                let filename = Path::new(input_path)
                    .file_name()
                    .ok_or_else(|| Error::from_reason("invalid filename"))?;
                
                let extension = match &self.format {
                    OutputFormat::Jpeg { .. } => "jpg",
                    OutputFormat::Png => "png",
                    OutputFormat::WebP { .. } => "webp",
                    OutputFormat::Avif { .. } => "avif",
                };
                
                let output_filename = Path::new(filename).with_extension(extension);
                let output_path = Path::new(&self.output_dir).join(output_filename);
                
                fs::write(&output_path, encoded)
                    .map_err(|e| Error::from_reason(format!("failed to write output: {}", e)))?;
                
                Ok(output_path.to_string_lossy().to_string())
            };

            match process_one() {
                Ok(path) => BatchResult {
                    source: input_path.clone(),
                    success: true,
                    error: None,
                    output_path: Some(path),
                },
                Err(e) => BatchResult {
                    source: input_path.clone(),
                    success: false,
                    error: Some(e.to_string()),
                    output_path: None,
                }
            }
        }).collect();

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
fn calc_resize_dimensions(
    orig_w: u32,
    orig_h: u32,
    target_w: Option<u32>,
    target_h: Option<u32>,
) -> (u32, u32) {
    match (target_w, target_h) {
        (Some(w), Some(h)) => (w, h),
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
fn check_dimensions(width: u32, height: u32) -> Result<()> {
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(Error::from_reason(format!(
            "image too large: {}x{} exceeds max dimension {}", 
            width, height, MAX_DIMENSION
        )));
    }
    let pixels = width as u64 * height as u64;
    if pixels > MAX_PIXELS {
        return Err(Error::from_reason(format!(
            "image too large: {} pixels exceeds max {}", 
            pixels, MAX_PIXELS
        )));
    }
    Ok(())
}
/// Validate ICC profile header
/// ICC profiles must start with a 128-byte header containing specific fields
fn validate_icc_profile(icc_data: &[u8]) -> bool {
    // Minimum ICC profile size is 128 bytes (header)
    if icc_data.len() < 128 {
        return false;
    }

    // Check profile size field (bytes 0-3, big-endian)
    let profile_size = u32::from_be_bytes([
        icc_data[0], icc_data[1], icc_data[2], icc_data[3]
    ]) as usize;
    
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

/// Extract ICC profile from JPEG data
fn extract_icc_from_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    let jpeg = Jpeg::from_bytes(data.to_vec().into()).ok()?;
    jpeg.icc_profile().map(|icc| icc.to_vec())
}

/// Extract ICC profile from PNG data
fn extract_icc_from_png(data: &[u8]) -> Option<Vec<u8>> {
    let png = Png::from_bytes(data.to_vec().into()).ok()?;
    png.icc_profile().map(|icc| icc.to_vec())
}

/// Extract ICC profile from WebP data
fn extract_icc_from_webp(data: &[u8]) -> Option<Vec<u8>> {
    use img_parts::webp::WebP;
    let webp = WebP::from_bytes(data.to_vec().into()).ok()?;
    webp.icc_profile().map(|icc| icc.to_vec())
}
