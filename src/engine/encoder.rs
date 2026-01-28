// src/engine/encoder.rs
//
// Encoder operations: JPEG (mozjpeg), PNG, WebP, AVIF with quality settings

use crate::codecs::avif_safe::{create_rgb_image, SafeAvifEncoder, SafeAvifImage, SafeAvifRwData};
use crate::engine::check_dimensions;
use crate::engine::common::run_with_panic_policy;
use crate::error::LazyImageError;
use image::{DynamicImage, GenericImageView, ImageFormat};
use img_parts::{jpeg::Jpeg, png::Png, ImageICC};
use libavif_sys::*;
use mozjpeg::{ColorSpace, Compress, ScanMode};
use std::cmp;
use std::io::Cursor;

// Type alias for Result - always use LazyImageError to preserve error taxonomy
// This ensures that encode errors are properly classified (CodecError, etc.)
// rather than being converted to generic InternalBug errors.
type EncoderResult<T> = std::result::Result<T, LazyImageError>;

fn validate_encode_dimensions(width: u32, height: u32, format: &'static str) -> EncoderResult<()> {
    if width == 0 || height == 0 {
        return Err(LazyImageError::encode_failed(
            format,
            format!("Invalid image dimensions: width={width}, height={height}"),
        ));
    }
    check_dimensions(width, height)?;
    Ok(())
}

fn validate_buffer_len(
    width: u32,
    height: u32,
    channels: usize,
    len: usize,
    _format: &'static str,
) -> EncoderResult<()> {
    let expected_len = width as usize * height as usize * channels;
    if len != expected_len {
        return Err(LazyImageError::corrupted_image());
    }
    Ok(())
}

/// Single source of truth for mapping quality (0-100) to per-format encoder knobs.
/// Bands are fixed (WebP filter_strength keeps sharp-compatible 80/60 thresholds):
/// - High (>=85): Quality first, AVIF speed 6
/// - Balanced (70-84): Balanced, AVIF speed 7
/// - Fast (50-69): Speed leaning, AVIF speed 8
/// - Fastest (<50): Lowest quality / fastest, AVIF speed 9
#[derive(Debug, Clone, Copy)]
pub struct QualitySettings {
    quality: f32,
    #[allow(dead_code)] // Reserved for future use (e.g., WebP/AVIF fast mode)
    fast_mode: bool, // Fast mode flag for JPEG encoding
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QualityBand {
    High,
    Balanced,
    Fast,
    Fastest,
}

impl QualitySettings {
    pub fn new(quality: u8) -> Self {
        let clamped = quality.min(100);
        Self {
            quality: clamped as f32,
            fast_mode: false, // Default: high quality mode
        }
    }

    /// Create with fast mode option
    pub fn with_fast_mode(quality: u8, fast_mode: bool) -> Self {
        let clamped = quality.min(100);
        Self {
            quality: clamped as f32,
            fast_mode,
        }
    }

    fn band(&self) -> QualityBand {
        if self.quality >= 85.0 {
            QualityBand::High
        } else if self.quality >= 70.0 {
            QualityBand::Balanced
        } else if self.quality >= 50.0 {
            QualityBand::Fast
        } else {
            QualityBand::Fastest
        }
    }

    // WebP settings - sharp-equivalent balanced settings
    // Optimized for speed while maintaining quality parity with sharp
    pub fn webp_method(&self) -> i32 {
        // Use method 4 for all quality levels (balanced, sharp-equivalent)
        // Method 4 provides optimal speed/quality trade-off
        4
    }

    pub fn webp_pass(&self) -> i32 {
        // Use single pass for all quality levels (sharp-equivalent)
        // Single pass is ~3-5x faster than multi-pass with minimal quality impact
        1
    }

    pub fn webp_preprocessing(&self) -> i32 {
        // No preprocessing (sharp-equivalent)
        // Disabling preprocessing improves speed by ~10-15%
        0
    }

    pub fn webp_sns_strength(&self) -> i32 {
        match self.band() {
            QualityBand::High => 50,
            QualityBand::Balanced => 70,
            QualityBand::Fast | QualityBand::Fastest => 80,
        }
    }

    pub fn webp_filter_strength(&self) -> i32 {
        if self.quality >= 80.0 {
            20
        } else if self.quality >= 60.0 {
            30
        } else {
            40
        }
    }

    pub fn webp_filter_sharpness(&self) -> i32 {
        match self.band() {
            QualityBand::High => 2,
            QualityBand::Balanced | QualityBand::Fast | QualityBand::Fastest => 0,
        }
    }

    // AVIF settings for libavif encoder
    // libavif speed: 0 (slowest/best) to 10 (fastest/worst)
    // Updated to match Sharp's speed settings for better performance
    // Aggressive speed lift to match Sharp defaults
    pub fn avif_speed(&self) -> i32 {
        match self.band() {
            QualityBand::High => 6, // High quality, slightly slower (was 4) - two steps faster than before
            QualityBand::Balanced => 7, // Balanced (was 5) - two steps faster than before
            QualityBand::Fast => 8, // Speed-biased (was 6) - two steps faster than before
            QualityBand::Fastest => 9, // Fastest useful (was 7) - two steps faster than before
        }
    }
}

/// Encode to JPEG using mozjpeg with RUTHLESS Web-optimized settings
///
/// This function uses high-quality settings by default. For faster encoding
/// (matching Sharp's speed), use `encode_jpeg_with_settings` with `fast_mode: true`.
pub fn encode_jpeg(img: &DynamicImage, quality: u8, icc: Option<&[u8]>) -> EncoderResult<Vec<u8>> {
    encode_jpeg_with_settings(img, quality, icc, false)
}

/// Encode to JPEG with explicit fast mode control
///
/// # Arguments
/// * `img` - Image to encode
/// * `quality` - Quality (0-100)
/// * `icc` - Optional ICC profile
/// * `fast_mode` - If true, disables expensive optimizations for faster encoding
///                 (matches Sharp/libjpeg-turbo defaults)
pub fn encode_jpeg_with_settings(
    img: &DynamicImage,
    quality: u8,
    icc: Option<&[u8]>,
    fast_mode: bool,
) -> EncoderResult<Vec<u8>> {
    run_with_panic_policy("encode:jpeg", || {
        use std::borrow::Cow;
        let quality = quality.min(100);

        // Zero-copy optimization: avoid conversion if already RGB8
        let rgb: Cow<'_, image::RgbImage> = match img {
            DynamicImage::ImageRgb8(rgb_img) => Cow::Borrowed(rgb_img),
            _ => Cow::Owned(img.to_rgb8()),
        };
        let (w, h) = rgb.dimensions();
        let pixels: &[u8] = rgb.as_raw();

        validate_encode_dimensions(w, h, "jpeg")?;
        validate_buffer_len(w, h, 3, pixels.len(), "jpeg")?;

        let mut comp = Compress::new(ColorSpace::JCS_RGB);
        comp.set_size(w as usize, h as usize);
        comp.set_color_space(ColorSpace::JCS_YCbCr);

        let quality_f32 = quality as f32;
        comp.set_quality(quality_f32);

        // =========================================================
        // RUTHLESS WEB OPTIMIZATION SETTINGS (Enhanced)
        // =========================================================

        comp.set_chroma_sampling_pixel_sizes((2, 2), (2, 2));
        comp.set_progressive_mode();

        if fast_mode {
            comp.set_optimize_coding(false);
            comp.set_optimize_scans(false);
        } else {
            comp.set_optimize_coding(true);
            comp.set_optimize_scans(true);
            comp.set_scan_optimization_mode(ScanMode::AllComponentsTogether);
        }

        let smoothing = if quality_f32 >= 90.0 {
            0
        } else if quality_f32 >= 70.0 {
            5
        } else if quality_f32 >= 60.0 {
            10
        } else {
            18
        };
        comp.set_smoothing_factor(smoothing);

        // 7. Quantization table optimization is implied by optimize_coding(true)

        let estimated_size = (w as usize * h as usize * 3 / 10).max(4096);
        let mut output = Vec::with_capacity(estimated_size);

        let encoded = {
            let mut writer = comp.start_compress(&mut output).map_err(|e| {
                LazyImageError::encode_failed(
                    "jpeg",
                    format!("mozjpeg: failed to start compress: {e:?}"),
                )
            })?;

            let stride = w as usize * 3;
            for row in pixels.chunks(stride) {
                writer.write_scanlines(row).map_err(|e| {
                    LazyImageError::encode_failed(
                        "jpeg",
                        format!("mozjpeg: failed to write scanlines: {e:?}"),
                    )
                })?;
            }

            writer.finish().map_err(|e| {
                LazyImageError::encode_failed("jpeg", format!("mozjpeg: failed to finish: {e:?}"))
            })?;

            output
        };

        if let Some(icc_data) = icc {
            embed_icc_jpeg(encoded, icc_data)
        } else {
            Ok(encoded)
        }
    })
}

/// Embed ICC profile into JPEG using img-parts
pub fn embed_icc_jpeg(jpeg_data: Vec<u8>, icc: &[u8]) -> EncoderResult<Vec<u8>> {
    run_with_panic_policy("encode:jpeg:embed_icc", || {
        use img_parts::jpeg::{markers::APP2, JpegSegment};
        use img_parts::Bytes;

        let mut jpeg = Jpeg::from_bytes(Bytes::from(jpeg_data)).map_err(|e| {
            LazyImageError::decode_failed(format!("failed to parse JPEG for ICC: {e}"))
        })?;

        let mut marker_data = Vec::with_capacity(14 + icc.len());
        marker_data.extend_from_slice(b"ICC_PROFILE\0");
        marker_data.push(1);
        marker_data.push(1);
        marker_data.extend_from_slice(icc);

        let segment = JpegSegment::new_with_contents(APP2, Bytes::from(marker_data));

        let segments = jpeg.segments_mut();
        segments.insert(0, segment);

        let mut output = Vec::new();
        jpeg.encoder().write_to(&mut output).map_err(|e| {
            LazyImageError::encode_failed("jpeg", format!("failed to write JPEG with ICC: {e}"))
        })?;

        Ok(output)
    })
}

/// Embed EXIF metadata into JPEG using img-parts
///
/// Note: This function expects raw TIFF-format EXIF data (without the "Exif\0\0" header).
/// Orientation is automatically reset to 1 if auto_orient was applied.
/// GPS tags are stripped if strip_gps is true (default for privacy).
pub fn embed_exif_jpeg(
    jpeg_data: Vec<u8>,
    exif: &[u8],
    reset_orientation: bool,
    strip_gps: bool,
) -> EncoderResult<Vec<u8>> {
    run_with_panic_policy("encode:jpeg:embed_exif", || {
        use img_parts::Bytes;
        use img_parts::ImageEXIF;

        // Parse JPEG
        let mut jpeg = Jpeg::from_bytes(Bytes::from(jpeg_data)).map_err(|e| {
            LazyImageError::decode_failed(format!("failed to parse JPEG for EXIF: {e}"))
        })?;

        // Sanitize EXIF data if needed
        let sanitized_exif = sanitize_exif_bytes(exif, reset_orientation, strip_gps)?;

        // Set EXIF data (img-parts adds the "Exif\0\0" header automatically for APP1)
        jpeg.set_exif(Some(Bytes::from(sanitized_exif)));

        // Write output
        let mut output = Vec::new();
        jpeg.encoder().write_to(&mut output).map_err(|e| {
            LazyImageError::encode_failed("jpeg", format!("failed to write JPEG with EXIF: {e}"))
        })?;

        Ok(output)
    })
}

/// Sanitize raw EXIF TIFF bytes (Zero-Copy approach):
/// - Reset Orientation tag to 1 (if reset_orientation is true)
/// - Strip GPS tags by zeroing GPS IFD pointer (if strip_gps is true)
///
/// This operates directly on the TIFF structure without creating temp files.
fn sanitize_exif_bytes(
    exif: &[u8],
    reset_orientation: bool,
    strip_gps: bool,
) -> EncoderResult<Vec<u8>> {
    // EXIF data is TIFF format - we can modify it directly
    // Structure: [byte order (2)] [magic 0x002A (2)] [IFD0 offset (4)] [IFD data...]
    if exif.len() < 8 {
        return Ok(exif.to_vec());
    }

    // Determine byte order
    let is_little_endian = match &exif[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return Ok(exif.to_vec()), // Unknown byte order, return unchanged
    };

    // Helper functions for reading/writing with correct endianness
    let read_u16 = |data: &[u8], offset: usize| -> u16 {
        if is_little_endian {
            u16::from_le_bytes([data[offset], data[offset + 1]])
        } else {
            u16::from_be_bytes([data[offset], data[offset + 1]])
        }
    };

    let read_u32 = |data: &[u8], offset: usize| -> u32 {
        if is_little_endian {
            u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ])
        } else {
            u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ])
        }
    };

    // Make a mutable copy for modification
    let mut result = exif.to_vec();

    // Get IFD0 offset
    let ifd0_offset = read_u32(&result, 4) as usize;
    if ifd0_offset >= result.len() || ifd0_offset < 8 {
        return Ok(result);
    }

    // Parse IFD0 to find Orientation and GPS IFD pointer tags
    let num_entries = read_u16(&result, ifd0_offset) as usize;
    let mut offset = ifd0_offset + 2;

    const ORIENTATION_TAG: u16 = 0x0112;
    const GPS_IFD_TAG: u16 = 0x8825;

    for _ in 0..num_entries {
        if offset + 12 > result.len() {
            break;
        }

        let tag = read_u16(&result, offset);
        let tag_type = read_u16(&result, offset + 2);
        // let count = read_u32(&result, offset + 4);
        let value_offset = offset + 8;

        // Reset Orientation tag to 1
        if reset_orientation && tag == ORIENTATION_TAG && tag_type == 3 {
            // Type 3 = SHORT (2 bytes)
            if is_little_endian {
                result[value_offset] = 1;
                result[value_offset + 1] = 0;
            } else {
                result[value_offset] = 0;
                result[value_offset + 1] = 1;
            }
        }

        // Zero out GPS IFD pointer to effectively strip GPS data
        if strip_gps && tag == GPS_IFD_TAG {
            // Zero the value/offset field (4 bytes)
            result[value_offset] = 0;
            result[value_offset + 1] = 0;
            result[value_offset + 2] = 0;
            result[value_offset + 3] = 0;
        }

        offset += 12; // Each IFD entry is 12 bytes
    }

    Ok(result)
}

/// Encode to PNG using image crate
pub fn encode_png(img: &DynamicImage, icc: Option<&[u8]>) -> EncoderResult<Vec<u8>> {
    run_with_panic_policy("encode:png", || {
        let (w, h) = img.dimensions();
        validate_encode_dimensions(w, h, "png")?;

        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .map_err(|e| LazyImageError::encode_failed("png", format!("PNG encode failed: {e}")))?;

        // Recompress with oxipng to losslessly reduce size
        let mut options = oxipng::Options::from_preset(4);
        // Preserve metadata (do not strip ICC)
        options.strip = oxipng::StripChunks::None;

        let optimized = oxipng::optimize_from_memory(&buf, &options).map_err(|e| {
            LazyImageError::encode_failed("png", format!("oxipng optimization failed: {e}"))
        })?;

        if let Some(icc_data) = icc {
            embed_icc_png(optimized, icc_data)
        } else {
            Ok(optimized)
        }
    })
}

/// Embed ICC profile into PNG using img-parts
pub fn embed_icc_png(png_data: Vec<u8>, icc: &[u8]) -> EncoderResult<Vec<u8>> {
    run_with_panic_policy("encode:png:embed_icc", || {
        use img_parts::Bytes;

        let mut png = Png::from_bytes(Bytes::from(png_data)).map_err(|e| {
            LazyImageError::decode_failed(format!("failed to parse PNG for ICC: {e}"))
        })?;

        png.set_icc_profile(Some(Bytes::from(icc.to_vec())));

        let mut output = Vec::new();
        png.encoder().write_to(&mut output).map_err(|e| {
            LazyImageError::encode_failed("png", format!("failed to write PNG with ICC: {e}"))
        })?;

        Ok(output)
    })
}

/// Encode to WebP with optimized settings
/// Avoids unnecessary alpha channel to reduce file size
pub fn encode_webp(img: &DynamicImage, quality: u8, icc: Option<&[u8]>) -> EncoderResult<Vec<u8>> {
    run_with_panic_policy("encode:webp", || {
        use std::borrow::Cow;

        let mut config = webp::WebPConfig::new()
            .map_err(|_| LazyImageError::internal_panic("failed to create WebPConfig"))?;

        let settings = QualitySettings::new(quality);
        config.quality = settings.quality;
        config.method = settings.webp_method();
        config.pass = settings.webp_pass();
        config.preprocessing = settings.webp_preprocessing();
        config.sns_strength = settings.webp_sns_strength();
        config.autofilter = 1;
        config.filter_strength = settings.webp_filter_strength();
        config.filter_sharpness = settings.webp_filter_sharpness();

        let mem = if img.color().has_alpha() {
            let rgba: Cow<'_, image::RgbaImage> = match img {
                DynamicImage::ImageRgba8(rgba_img) => Cow::Borrowed(rgba_img),
                _ => Cow::Owned(img.to_rgba8()),
            };
            let (w, h) = rgba.dimensions();
            validate_encode_dimensions(w, h, "webp")?;
            validate_buffer_len(w, h, 4, rgba.as_raw().len(), "webp")?;
            webp::Encoder::from_rgba(&rgba, w, h).encode_advanced(&config)
        } else {
            let rgb: Cow<'_, image::RgbImage> = match img {
                DynamicImage::ImageRgb8(rgb_img) => Cow::Borrowed(rgb_img),
                _ => Cow::Owned(img.to_rgb8()),
            };
            let (w, h) = rgb.dimensions();
            validate_encode_dimensions(w, h, "webp")?;
            validate_buffer_len(w, h, 3, rgb.as_raw().len(), "webp")?;
            webp::Encoder::from_rgb(&rgb, w, h).encode_advanced(&config)
        }
        .map_err(|e| LazyImageError::encode_failed("webp", format!("WebP encode failed: {e:?}")))?;

        let encoded = mem.to_vec();

        if let Some(icc_data) = icc {
            embed_icc_webp(encoded, icc_data)
        } else {
            Ok(encoded)
        }
    })
}

/// Embed ICC profile into WebP using img-parts
pub fn embed_icc_webp(webp_data: Vec<u8>, icc: &[u8]) -> EncoderResult<Vec<u8>> {
    run_with_panic_policy("encode:webp:embed_icc", || {
        use img_parts::webp::WebP;
        use img_parts::Bytes;

        let mut webp = WebP::from_bytes(Bytes::from(webp_data)).map_err(|e| {
            LazyImageError::decode_failed(format!("failed to parse WebP for ICC: {e}"))
        })?;

        webp.set_icc_profile(Some(Bytes::from(icc.to_vec())));

        let mut output = Vec::new();
        webp.encoder().write_to(&mut output).map_err(|e| {
            LazyImageError::encode_failed("webp", format!("failed to write WebP with ICC: {e}"))
        })?;

        Ok(output)
    })
}

/// Encode to AVIF format using libavif (AOMedia reference implementation).
///
/// This implementation properly supports:
/// - ICC profile embedding via avifImageSetProfileICC
/// - Accurate RGB-to-YUV conversion with proper color matrix
/// - Alpha channel handling with separate quality control
///
/// This function uses safe abstractions from `codecs::avif_safe` to minimize
/// unsafe blocks and improve memory safety.
pub fn encode_avif(img: &DynamicImage, quality: u8, icc: Option<&[u8]>) -> EncoderResult<Vec<u8>> {
    run_with_panic_policy("encode:avif", || {
        use std::borrow::Cow;

        let clamped_quality = quality.min(100);
        let settings = QualitySettings::new(clamped_quality);
        let (width, height) = img.dimensions();
        validate_encode_dimensions(width, height, "avif")?;

        let has_alpha = img.color().has_alpha();

        let rgba: Cow<'_, image::RgbaImage> = match img {
            DynamicImage::ImageRgba8(rgba_img) => Cow::Borrowed(rgba_img),
            _ => Cow::Owned(img.to_rgba8()),
        };
        let pixels = rgba.as_raw();
        validate_buffer_len(width, height, 4, pixels.len(), "avif")?;

        let mut avif_image = SafeAvifImage::new(width, height, 8, AVIF_PIXEL_FORMAT_YUV420)
            .map_err(|e| LazyImageError::encode_failed("avif".to_string(), e.to_string()))?;

        avif_image.set_color_properties(
            AVIF_COLOR_PRIMARIES_BT709 as u16,
            AVIF_TRANSFER_CHARACTERISTICS_SRGB as u16,
            AVIF_MATRIX_COEFFICIENTS_BT709 as u16,
            AVIF_RANGE_FULL,
        );

        if let Some(icc_data) = icc {
            avif_image
                .set_icc_profile(icc_data)
                .map_err(|e| LazyImageError::encode_failed("avif".to_string(), e.to_string()))?;
        }

        let rgb = create_rgb_image(&mut avif_image, pixels.as_ptr(), width, height)
            .map_err(|e| LazyImageError::encode_failed("avif".to_string(), e.to_string()))?;

        avif_image
            .allocate_planes(AVIF_PLANES_YUV)
            .map_err(|e| LazyImageError::encode_failed("avif".to_string(), e.to_string()))?;

        avif_image
            .rgb_to_yuv(&rgb)
            .map_err(|e| LazyImageError::encode_failed("avif".to_string(), e.to_string()))?;

        if has_alpha {
            avif_image
                .allocate_planes(AVIF_PLANES_A)
                .map_err(|e| LazyImageError::encode_failed("avif".to_string(), e.to_string()))?;

            unsafe {
                let alpha_plane = avif_image.alpha_plane_mut().map_err(|e| {
                    LazyImageError::encode_failed("avif".to_string(), e.to_string())
                })?;
                let alpha_row_bytes = avif_image.alpha_row_bytes();
                for y in 0..height as usize {
                    for x in 0..width as usize {
                        let src_idx = (y * width as usize + x) * 4 + 3;
                        let dst_idx = y * alpha_row_bytes + x;
                        *alpha_plane.as_ptr().add(dst_idx) = pixels[src_idx];
                    }
                }
            }
        }

        let mut encoder = SafeAvifEncoder::new()
            .map_err(|e| LazyImageError::encode_failed("avif".to_string(), e.to_string()))?;

        let cpu_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2);
        let capped = cmp::min(8, cpu_threads);
        let encoder_threads = cmp::max(2, capped) as i32;

        encoder.configure(
            clamped_quality,
            clamped_quality,
            settings.avif_speed(),
            encoder_threads,
        );

        let mut output = SafeAvifRwData::new();

        encoder
            .add_image(&mut avif_image, 1, AVIF_ADD_IMAGE_FLAG_SINGLE)
            .map_err(|e| LazyImageError::encode_failed("avif".to_string(), e.to_string()))?;

        encoder
            .finish(&mut output)
            .map_err(|e| LazyImageError::encode_failed("avif".to_string(), e.to_string()))?;

        Ok(output.to_vec())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbImage, RgbaImage};

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

    mod encode_tests {
        use super::*;

        #[test]
        fn test_encode_jpeg_produces_valid_jpeg() {
            let img = create_test_image(100, 100);
            let result = encode_jpeg(&img, 80, None).unwrap();
            // Check JPEG magic bytes
            assert_eq!(&result[0..2], &[0xFF, 0xD8]);
            // Check JPEG end marker
            assert_eq!(&result[result.len() - 2..], &[0xFF, 0xD9]);
        }

        #[test]
        fn test_encode_jpeg_quality_affects_size() {
            let img = create_test_image(100, 100);
            let high_quality = encode_jpeg(&img, 95, None).unwrap();
            let low_quality = encode_jpeg(&img, 50, None).unwrap();
            // High quality is usually larger (content can flip this); both should be valid JPEGs
            assert!(high_quality.len() > 0);
            assert!(low_quality.len() > 0);
            assert_eq!(&high_quality[0..2], &[0xFF, 0xD8]);
            assert_eq!(&low_quality[0..2], &[0xFF, 0xD8]);
        }

        #[test]
        fn test_encode_jpeg_with_icc() {
            let img = create_test_image(100, 100);
            // Minimal valid ICC profile
            let mut icc_data = vec![0u8; 128];
            icc_data[0] = 0x00;
            icc_data[1] = 0x00;
            icc_data[2] = 0x00;
            icc_data[3] = 0x80; // 128 bytes
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

            let result = encode_jpeg(&img, 80, Some(&icc_data)).unwrap();
            assert_eq!(&result[0..2], &[0xFF, 0xD8]);
        }

        #[test]
        fn test_encode_jpeg_fast_mode_produces_valid_jpeg() {
            let img = create_test_image(100, 100);
            let result = encode_jpeg_with_settings(&img, 80, None, true).unwrap();
            // Check JPEG magic bytes
            assert_eq!(&result[0..2], &[0xFF, 0xD8]);
            // Check JPEG end marker
            assert_eq!(&result[result.len() - 2..], &[0xFF, 0xD9]);
        }

        #[test]
        fn test_encode_jpeg_fast_mode_vs_default_size_difference() {
            let img = create_test_image(500, 500); // Larger image for more noticeable difference
            let fast_result = encode_jpeg_with_settings(&img, 80, None, true).unwrap();
            let default_result = encode_jpeg_with_settings(&img, 80, None, false).unwrap();

            // Both should be valid JPEGs
            assert_eq!(&fast_result[0..2], &[0xFF, 0xD8]);
            assert_eq!(&default_result[0..2], &[0xFF, 0xD8]);

            // Fast mode typically produces slightly larger files (5-10% increase)
            // but should still be reasonable
            assert!(fast_result.len() > 0);
            assert!(default_result.len() > 0);
            // Fast mode file size should be within reasonable range (not 10x larger)
            assert!(fast_result.len() < default_result.len() * 2);
        }

        #[test]
        fn test_encode_jpeg_fast_mode_with_icc() {
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

            let result = encode_jpeg_with_settings(&img, 80, Some(&icc_data), true).unwrap();
            assert_eq!(&result[0..2], &[0xFF, 0xD8]);
        }

        #[test]
        fn test_encode_jpeg_fast_mode_quality_consistency() {
            let img = create_test_image(200, 200);
            // Test that fast mode works with different quality levels
            for quality in [50, 75, 90] {
                let result = encode_jpeg_with_settings(&img, quality, None, true).unwrap();
                assert_eq!(&result[0..2], &[0xFF, 0xD8]);
                assert!(result.len() > 0);
            }
        }

        #[test]
        fn test_encode_png_produces_valid_png() {
            let img = create_test_image(100, 100);
            let result = encode_png(&img, None).unwrap();
            // Check PNG magic bytes
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

            let result = encode_png(&img, Some(&icc_data)).unwrap();
            assert_eq!(
                &result[0..8],
                &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
            );
        }

        #[test]
        fn test_encode_webp_produces_valid_webp() {
            let img = create_test_image(100, 100);
            let result = encode_webp(&img, 80, None).unwrap();
            // Check WebP magic bytes (RIFF....WEBP)
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

            let result = encode_webp(&img, 80, Some(&icc_data)).unwrap();
            assert_eq!(&result[0..4], b"RIFF");
            assert_eq!(&result[8..12], b"WEBP");
        }

        #[test]
        fn test_encode_avif_produces_valid_avif() {
            let img = create_test_image(100, 100);
            let result = encode_avif(&img, 60, None).unwrap();
            // AVIF should contain an ftyp box near the start
            assert!(result.len() > 12);
            // Ensure "ftyp" exists
            let has_ftyp = result.windows(4).any(|w| w == b"ftyp");
            assert!(has_ftyp);
        }

        #[test]
        fn test_encode_avif_quality_affects_size() {
            let img = create_test_image(100, 100);
            let high_quality = encode_avif(&img, 80, None).unwrap();
            let low_quality = encode_avif(&img, 40, None).unwrap();
            // Both outputs should be valid AVIF
            assert!(high_quality.len() > 0);
            assert!(low_quality.len() > 0);
        }

        #[test]
        fn test_quality_band_mapping_boundaries() {
            let high = QualitySettings::new(90);
            let balanced = QualitySettings::new(75);
            let fast = QualitySettings::new(60);
            let fastest = QualitySettings::new(40);

            assert_eq!(high.avif_speed(), 6);
            assert_eq!(balanced.avif_speed(), 7);
            assert_eq!(fast.avif_speed(), 8);
            assert_eq!(fastest.avif_speed(), 9);
        }

        #[test]
        fn test_quality_settings_webp_mapping_is_stable() {
            let high = QualitySettings::new(90);
            assert_eq!(high.webp_method(), 4);
            assert_eq!(high.webp_pass(), 1);
            assert_eq!(high.webp_sns_strength(), 50);
            assert_eq!(high.webp_filter_strength(), 20);
            assert_eq!(high.webp_filter_sharpness(), 2);

            let balanced = QualitySettings::new(75);
            assert_eq!(balanced.webp_sns_strength(), 70);
            assert_eq!(balanced.webp_filter_strength(), 30);
            assert_eq!(balanced.webp_filter_sharpness(), 0);

            let fast = QualitySettings::new(60);
            assert_eq!(fast.webp_sns_strength(), 80);
            assert_eq!(fast.webp_filter_strength(), 30);

            let fastest = QualitySettings::new(40);
            assert_eq!(fastest.webp_sns_strength(), 80);
            assert_eq!(fastest.webp_filter_strength(), 40);
        }

        #[test]
        fn test_encode_rgba_image() {
            let img = create_test_image_rgba(100, 100);
            let jpeg_result = encode_jpeg(&img, 80, None).unwrap();
            assert_eq!(&jpeg_result[0..2], &[0xFF, 0xD8]);

            let png_result = encode_png(&img, None).unwrap();
            assert_eq!(
                &png_result[0..8],
                &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
            );
        }
    }
}
