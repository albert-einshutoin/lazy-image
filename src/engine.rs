// src/engine.rs
//
// The core of lazy-image. A lazy pipeline that:
// 1. Queues operations without executing
// 2. Runs everything in a single pass on compute()
// 3. Uses NAPI AsyncTask to not block Node.js main thread

use crate::ops::{Operation, OutputFormat};
use fast_image_resize::{self as fir, PixelType, ResizeOptions};
use image::{DynamicImage, GenericImageView, ImageFormat, RgbImage, RgbaImage};
use img_parts::{jpeg::Jpeg, png::Png, ImageICC};
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
    /// ICC color profile extracted from source image
    icc_profile: Option<Vec<u8>>,
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
        let icc_profile = extract_icc_profile(&data);
        
        ImageEngine {
            source: Some(data),
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
        let icc_profile = extract_icc_profile(&data);

        Ok(ImageEngine {
            source: Some(data),
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
        let icc_profile = self.icc_profile.take();

        Ok(AsyncTask::new(EncodeTask {
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
    #[napi(js_name = "toFile")]
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
    icc_profile: Option<Vec<u8>>,
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
    fn encode_avif(img: &DynamicImage, quality: u8, _icc: Option<&[u8]>) -> Result<Vec<u8>> {
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
        let encoder = AvifEncoder::new()
            .with_quality(quality as f32)
            .with_speed(6);  // 1-10, higher = faster but larger

        // Note: ravif 0.11 doesn't have native ICC embedding API
        // AVIF files assume sRGB by default, which is acceptable for web use
        // TODO: Consider using libavif bindings for full ICC support in the future

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

        // 3. Encode with ICC profile preservation
        let icc = self.icc_profile.as_deref();
        match &self.format {
            OutputFormat::Jpeg { quality } => Self::encode_jpeg(&processed, *quality, icc),
            OutputFormat::Png => Self::encode_png(&processed, icc),
            OutputFormat::WebP { quality } => Self::encode_webp(&processed, *quality, icc),
            OutputFormat::Avif { quality } => Self::encode_avif(&processed, *quality, icc),
        }
    }

    fn resolve(&mut self, env: Env, output: Self::Output) -> Result<Self::JsValue> {
        env.create_buffer_with_data(output).map(|b| b.into_raw())
    }
}

// =============================================================================
// WRITE FILE TASK - File output without touching Node.js heap
// =============================================================================

pub struct WriteFileTask {
    source: Option<Vec<u8>>,
    decoded: Option<DynamicImage>,
    ops: Vec<Operation>,
    format: OutputFormat,
    icc_profile: Option<Vec<u8>>,
    output_path: String,
}

#[napi]
impl Task for WriteFileTask {
    type Output = u32; // Bytes written
    type JsValue = u32;

    fn compute(&mut self) -> Result<Self::Output> {
        use std::fs::File;
        use std::io::Write;

        // Reuse EncodeTask's processing logic
        let encode_task = EncodeTask {
            source: self.source.take(),
            decoded: self.decoded.take(),
            ops: std::mem::take(&mut self.ops),
            format: self.format.clone(),
            icc_profile: self.icc_profile.take(),
        };

        // 1. Decode
        let img = encode_task.decode()?;

        // 2. Apply operations
        let processed = EncodeTask::apply_ops(img, &encode_task.ops);

        // 3. Encode with ICC profile preservation
        let icc = encode_task.icc_profile.as_deref();
        let data = match &encode_task.format {
            OutputFormat::Jpeg { quality } => EncodeTask::encode_jpeg(&processed, *quality, icc)?,
            OutputFormat::Png => EncodeTask::encode_png(&processed, icc)?,
            OutputFormat::WebP { quality } => EncodeTask::encode_webp(&processed, *quality, icc)?,
            OutputFormat::Avif { quality } => EncodeTask::encode_avif(&processed, *quality, icc)?,
        };

        // 4. Write to file
        let mut file = File::create(&self.output_path)
            .map_err(|e| Error::from_reason(format!("failed to create file '{}': {}", self.output_path, e)))?;
        
        let bytes_written = data.len() as u32;
        file.write_all(&data)
            .map_err(|e| Error::from_reason(format!("failed to write file '{}': {}", self.output_path, e)))?;
        
        Ok(bytes_written)
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
fn extract_icc_profile(data: &[u8]) -> Option<Vec<u8>> {
    // Check magic bytes to determine format
    if data.len() < 12 {
        return None;
    }

    // JPEG: starts with 0xFF 0xD8
    if data[0] == 0xFF && data[1] == 0xD8 {
        return extract_icc_from_jpeg(data);
    }

    // PNG: starts with 0x89 0x50 0x4E 0x47
    if data[0] == 0x89 && data[1] == 0x50 && data[2] == 0x4E && data[3] == 0x47 {
        return extract_icc_from_png(data);
    }

    // WebP: starts with "RIFF" then 4 bytes size then "WEBP"
    if &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return extract_icc_from_webp(data);
    }

    None
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
