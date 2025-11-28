// src/engine.rs
//
// The core of lazy-image. A lazy pipeline that:
// 1. Queues operations without executing
// 2. Runs everything in a single pass on compute()
// 3. Uses NAPI AsyncTask to not block Node.js main thread

use crate::ops::{Operation, OutputFormat};
use fast_image_resize::{self as fir, PixelType, ResizeOptions};
use image::{DynamicImage, GenericImageView, ImageFormat, RgbImage, RgbaImage};
use mozjpeg::{ColorSpace, Compress, Decompress, ScanMode};
use napi::bindgen_prelude::*;
use napi::{Env, JsBuffer, Task};
use ravif::{Encoder as AvifEncoder, Img};
use rgb::FromSlice;
use std::io::Cursor;
use std::panic;

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
    source: Option<Vec<u8>>,
    /// Decoded image (populated after first decode or on sync operations)
    decoded: Option<DynamicImage>,
    /// Queued operations
    ops: Vec<Operation>,
}

#[napi]
impl ImageEngine {
    // =========================================================================
    // CONSTRUCTORS
    // =========================================================================

    /// Create engine from a buffer. Decoding is lazy.
    #[napi(factory)]
    pub fn from(buffer: Buffer) -> Self {
        ImageEngine {
            source: Some(buffer.to_vec()),
            decoded: None,
            ops: Vec::new(),
        }
    }

    /// Create a clone of this engine (for multi-output scenarios)
    #[napi(js_name = "clone")]
    pub fn clone_engine(&self) -> Result<ImageEngine> {
        Ok(ImageEngine {
            source: self.source.clone(),
            decoded: self.decoded.clone(),
            ops: self.ops.clone(),
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

    // =========================================================================
    // OUTPUT - Triggers async computation
    // =========================================================================

    /// Encode to buffer asynchronously.
    /// format: "jpeg", "jpg", "png", "webp"
    /// quality: 1-100 (default 80, ignored for PNG)
    #[napi]
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

        Ok(AsyncTask::new(EncodeTask {
            source,
            decoded,
            ops,
            format: output_format,
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
}

#[napi(object)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
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
            
            self.decoded = Some(img);
        }
        
        // Safe because we just set it above
        Ok(self.decoded.as_ref().unwrap())
    }
}

// =============================================================================
// ASYNC TASK - Where the real work happens
// =============================================================================

pub struct EncodeTask {
    source: Option<Vec<u8>>,
    decoded: Option<DynamicImage>,
    ops: Vec<Operation>,
    format: OutputFormat,
}

impl EncodeTask {
    /// Decode image from source bytes
    /// Uses mozjpeg (libjpeg-turbo) for JPEG, falls back to image crate for others
    fn decode(&self) -> Result<DynamicImage> {
        // Prefer already decoded image
        if let Some(ref img) = self.decoded {
            return Ok(img.clone());
        }

        let source = self.source.as_ref()
            .ok_or_else(|| Error::from_reason("no image source"))?;

        // Check magic bytes for JPEG (0xFF 0xD8)
        if source.len() >= 2 && source[0] == 0xFF && source[1] == 0xD8 {
            // JPEG detected - use mozjpeg for TURBO speed
            Self::decode_jpeg_mozjpeg(source)
        } else {
            // PNG, WebP, etc - use image crate
            image::load_from_memory(source)
                .map_err(|e| Error::from_reason(format!("decode failed: {e}")))
        }
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

    /// Apply all queued operations
    fn apply_ops(mut img: DynamicImage, ops: &[Operation]) -> DynamicImage {
        for op in ops {
            img = match op {
                Operation::Resize { width, height } => {
                    let (w, h) = calc_resize_dimensions(
                        img.width(), 
                        img.height(), 
                        *width, 
                        *height
                    );
                    // Use SIMD-accelerated fast_image_resize
                    Self::fast_resize(&img, w, h).unwrap_or_else(|_| {
                        // Fallback to image crate if fast_image_resize fails
                        img.resize_exact(w, h, image::imageops::FilterType::Lanczos3)
                    })
                }

                Operation::Crop { x, y, width, height } => {
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
                        _ => img, // Ignore invalid rotations
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
            };
        }
        img
    }

    /// SIMD-accelerated resize using fast_image_resize
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
    fn encode_jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let pixels = rgb.into_raw();

        // mozjpeg can panic internally, so we catch it
        let result = panic::catch_unwind(|| {
            let mut comp = Compress::new(ColorSpace::JCS_RGB);
            
            comp.set_size(w as usize, h as usize);
            
            // Output color space: YCbCr (standard for JPEG)
            comp.set_color_space(ColorSpace::JCS_YCbCr);
            
            // Quality setting
            comp.set_quality(quality as f32);
            
            // =========================================================
            // RUTHLESS WEB OPTIMIZATION SETTINGS
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
            
            // 5. Smoothing: Reduces high-frequency noise for better compression
            //    Value 0-100, higher = more smoothing. 10-20 is subtle.
            comp.set_smoothing_factor(10);

            let mut output = Vec::new();
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

        result.map_err(|_| Error::from_reason("mozjpeg panicked during encoding"))
    }

    /// Encode to PNG using image crate
    fn encode_png(img: &DynamicImage) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .map_err(|e| Error::from_reason(format!("PNG encode failed: {e}")))?;
        Ok(buf)
    }

    /// Encode to WebP with optimized settings
    fn encode_webp(img: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
        // Use RGB instead of RGBA for smaller files
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();

        let encoder = webp::Encoder::from_rgb(&rgb, w, h);
        
        // Create WebPConfig with preprocessing for better compression
        let mut config = webp::WebPConfig::new()
            .map_err(|_| Error::from_reason("failed to create WebPConfig"))?;
        
        config.quality = quality as f32;
        config.method = 6;  // Maximum compression effort
        
        // More encoding passes for better compression
        config.pass = 4;
        
        // Preprocessing: segment-smooth to reduce noise before encoding
        config.preprocessing = 1;
        
        // Higher spatial noise shaping for better compression
        config.sns_strength = 70;
        
        // Let libwebp auto-adjust filter
        config.autofilter = 1;
        
        let mem = encoder.encode_advanced(&config)
            .map_err(|e| Error::from_reason(format!("WebP encode failed: {e:?}")))?;
        
        Ok(mem.to_vec())
    }

    /// Encode to AVIF - next-gen format, even smaller than WebP
    fn encode_avif(img: &DynamicImage, quality: u8) -> Result<Vec<u8>> {
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let pixels = rgba.as_raw();

        // Convert to ravif's expected format
        let img_ref = Img::new(
            pixels.as_rgba(),
            width as usize,
            height as usize,
        );

        // Create encoder with quality setting
        // ravif quality: 0-100 (higher = better quality, larger file)
        // For compression, we invert: lower quality = smaller file
        let encoder = AvifEncoder::new()
            .with_quality(quality as f32)
            .with_speed(6);  // 1-10, higher = faster but larger

        let result = encoder.encode_rgba(img_ref)
            .map_err(|e| Error::from_reason(format!("AVIF encode failed: {e}")))?;

        Ok(result.avif_file)
    }
}

#[napi]
impl Task for EncodeTask {
    type Output = Vec<u8>;
    type JsValue = JsBuffer;

    fn compute(&mut self) -> Result<Self::Output> {
        // 1. Decode
        let img = self.decode()?;

        // 2. Apply operations
        let processed = Self::apply_ops(img, &self.ops);

        // 3. Encode
        match &self.format {
            OutputFormat::Jpeg { quality } => Self::encode_jpeg(&processed, *quality),
            OutputFormat::Png => Self::encode_png(&processed),
            OutputFormat::WebP { quality } => Self::encode_webp(&processed, *quality),
            OutputFormat::Avif { quality } => Self::encode_avif(&processed, *quality),
        }
    }

    fn resolve(&mut self, env: Env, output: Self::Output) -> Result<Self::JsValue> {
        env.create_buffer_with_data(output).map(|b| b.into_raw())
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
