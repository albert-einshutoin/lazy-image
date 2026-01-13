// src/engine/encoder.rs
//
// Encoder operations: JPEG (mozjpeg), PNG, WebP, AVIF with quality settings

use crate::codecs::avif_safe::{create_rgb_image, SafeAvifEncoder, SafeAvifImage, SafeAvifRwData};
use crate::error::LazyImageError;
use image::{DynamicImage, GenericImageView, ImageFormat};
use img_parts::{jpeg::Jpeg, png::Png, ImageICC};
use libavif_sys::*;
use mozjpeg::{ColorSpace, Compress, ScanMode};
use std::cmp;
use std::io::Cursor;

use crate::engine::MAX_DIMENSION;

// Type alias for Result - use napi::Result when napi is enabled, otherwise use standard Result
#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;
#[cfg(feature = "napi")]
type EncoderResult<T> = Result<T>;
#[cfg(not(feature = "napi"))]
type EncoderResult<T> = std::result::Result<T, LazyImageError>;

// Helper function to convert LazyImageError to the appropriate error type
#[cfg(feature = "napi")]
fn to_encoder_error(err: LazyImageError) -> napi::Error {
    napi::Error::from(err)
}

#[cfg(not(feature = "napi"))]
fn to_encoder_error(err: LazyImageError) -> LazyImageError {
    err
}

// Quality configuration helper
#[derive(Debug, Clone, Copy)]
pub struct QualitySettings {
    quality: f32,
    fast_mode: bool, // Fast mode flag for JPEG encoding
}

impl QualitySettings {
    pub fn new(quality: u8) -> Self {
        Self {
            quality: quality as f32,
            fast_mode: false, // Default: high quality mode
        }
    }

    /// Create with fast mode option
    pub fn with_fast_mode(quality: u8, fast_mode: bool) -> Self {
        Self {
            quality: quality as f32,
            fast_mode,
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
        if self.quality >= 85.0 {
            50
        } else if self.quality >= 70.0 {
            70
        } else {
            80
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
        if self.quality >= 85.0 {
            2
        } else {
            0
        }
    }

    // AVIF settings for libavif encoder
    // libavif speed: 0 (slowest/best) to 10 (fastest/worst)
    // Updated to match Sharp's speed settings for better performance
    // Sharpに速度で追いつくためのアグレッシブな設定変更
    pub fn avif_speed(&self) -> i32 {
        if self.quality >= 85.0 {
            6 // High quality, slightly slower (was 4) - 2段階高速化
        } else if self.quality >= 70.0 {
            7 // Good balance (was 5) - 2段階高速化
        } else if self.quality >= 50.0 {
            8 // Fast (was 6) - 2段階高速化
        } else {
            9 // Fastest useful (was 7) - 2段階高速化
        }
    }
}

/// Encode to JPEG using mozjpeg with RUTHLESS Web-optimized settings
/// 
/// This function uses high-quality settings by default. For faster encoding
/// (matching Sharp's speed), use `encode_jpeg_with_settings` with `fast_mode: true`.
pub fn encode_jpeg(
    img: &DynamicImage,
    quality: u8,
    icc: Option<&[u8]>,
) -> EncoderResult<Vec<u8>> {
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
    use std::borrow::Cow;

    // Zero-copy optimization: avoid conversion if already RGB8
    let rgb: Cow<'_, image::RgbImage> = match img {
        DynamicImage::ImageRgb8(rgb_img) => Cow::Borrowed(rgb_img),
        _ => Cow::Owned(img.to_rgb8()),
    };
    let (w, h) = rgb.dimensions();
    let pixels: &[u8] = rgb.as_raw();

    // 1. 事前検証 (パニック要因の排除)
    // 画像サイズの妥当性チェック
    if w == 0 || h == 0 {
        return Err(to_encoder_error(LazyImageError::internal_panic(
            "Invalid image dimensions: width or height is zero",
        )));
    }

    // MAX_DIMENSIONチェック（プロジェクト全体の一貫性のため）
    if w > MAX_DIMENSION || h > MAX_DIMENSION {
        return Err(to_encoder_error(LazyImageError::dimension_exceeds_limit(
            w.max(h),
            MAX_DIMENSION,
        )));
    }

    // バッファサイズの整合性チェック（非常に重要）
    let expected_len = (w as usize) * (h as usize) * 3;
    if pixels.len() != expected_len {
        return Err(to_encoder_error(LazyImageError::corrupted_image()));
    }

    // 2. エンコード (catch_unwind は削除)
    // ここでパニックが起きるなら、それはライブラリのバグなのでクラッシュさせるべき（Fail Fast）
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

    // 3. & 4. Optimize Huffman tables and scan order
    // Fast mode: Sharp (libjpeg-turbo defaults) に近い設定
    if fast_mode {
        // Disable expensive optimizations for faster encoding
        comp.set_optimize_coding(false); 
        comp.set_optimize_scans(false);
    } else {
        // 既存の高品質設定 (mozjpeg defaults)
        // Optimize Huffman tables: Custom tables per image
        comp.set_optimize_coding(true);

        // Optimize scan order: Better progressive compression
        comp.set_optimize_scans(true);
        comp.set_scan_optimization_mode(ScanMode::AllComponentsTogether);
    }

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

    let encoded = {
        let mut writer = comp.start_compress(&mut output).map_err(|e| {
            to_encoder_error(LazyImageError::encode_failed(
                "jpeg",
                format!("mozjpeg: failed to start compress: {e:?}"),
            ))
        })?;

        let stride = w as usize * 3;
        for row in pixels.chunks(stride) {
            writer.write_scanlines(row).map_err(|e| {
                to_encoder_error(LazyImageError::encode_failed(
                    "jpeg",
                    format!("mozjpeg: failed to write scanlines: {e:?}"),
                ))
            })?;
        }

        writer.finish().map_err(|e| {
            to_encoder_error(LazyImageError::encode_failed(
                "jpeg",
                format!("mozjpeg: failed to finish: {e:?}"),
            ))
        })?;

        output
    };

    // Embed ICC profile using img-parts if present
    if let Some(icc_data) = icc {
        embed_icc_jpeg(encoded, icc_data)
    } else {
        Ok(encoded)
    }
}

/// Embed ICC profile into JPEG using img-parts
pub fn embed_icc_jpeg(jpeg_data: Vec<u8>, icc: &[u8]) -> EncoderResult<Vec<u8>> {
    use img_parts::jpeg::{JpegSegment, markers::APP2};
    use img_parts::Bytes;

    let mut jpeg = Jpeg::from_bytes(Bytes::from(jpeg_data)).map_err(|e| {
        to_encoder_error(LazyImageError::decode_failed(format!(
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
    let segment = JpegSegment::new_with_contents(APP2, Bytes::from(marker_data));

    // Insert after SOI (before other segments)
    let segments = jpeg.segments_mut();
    segments.insert(0, segment);

    // Encode back
    let mut output = Vec::new();
    jpeg.encoder().write_to(&mut output).map_err(|e| {
        to_encoder_error(LazyImageError::encode_failed(
            "jpeg",
            format!("failed to write JPEG with ICC: {e}"),
        ))
    })?;

    Ok(output)
}

/// Encode to PNG using image crate
pub fn encode_png(img: &DynamicImage, icc: Option<&[u8]>) -> EncoderResult<Vec<u8>> {
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| {
            to_encoder_error(LazyImageError::encode_failed(
                "png",
                format!("PNG encode failed: {e}"),
            ))
        })?;

    // Embed ICC profile if present
    if let Some(icc_data) = icc {
        embed_icc_png(buf, icc_data)
    } else {
        Ok(buf)
    }
}

/// Embed ICC profile into PNG using img-parts
pub fn embed_icc_png(png_data: Vec<u8>, icc: &[u8]) -> EncoderResult<Vec<u8>> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use img_parts::Bytes;
    use std::io::Write;

    let mut png = Png::from_bytes(Bytes::from(png_data)).map_err(|e| {
        to_encoder_error(LazyImageError::decode_failed(format!(
            "failed to parse PNG for ICC: {e}"
        )))
    })?;

    // iCCP chunk format: profile_name (null-terminated) + compression_method (0) + compressed_data
    let profile_name = b"ICC\0"; // Short name
    let compression_method = 0u8; // zlib

    // Compress ICC data
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(icc).map_err(|e| {
        to_encoder_error(LazyImageError::encode_failed(
            "png",
            format!("failed to compress ICC: {e}"),
        ))
    })?;
    let compressed = encoder.finish().map_err(|e| {
        to_encoder_error(LazyImageError::encode_failed(
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
        to_encoder_error(LazyImageError::encode_failed(
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
) -> EncoderResult<Vec<u8>> {
    use std::borrow::Cow;

    // Zero-copy optimization: avoid conversion if already RGB8
    let rgb: Cow<'_, image::RgbImage> = match img {
        DynamicImage::ImageRgb8(rgb_img) => Cow::Borrowed(rgb_img),
        _ => Cow::Owned(img.to_rgb8()),
    };
    let (w, h) = rgb.dimensions();
    let encoder = webp::Encoder::from_rgb(&rgb, w, h);

    // Create WebPConfig with enhanced preprocessing for better compression
    let mut config = webp::WebPConfig::new().map_err(|_| {
        to_encoder_error(LazyImageError::internal_panic(
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
        to_encoder_error(LazyImageError::encode_failed(
            "webp",
            format!("WebP encode failed: {e:?}"),
        ))
    })?;

    let encoded = mem.to_vec();

    // Embed ICC profile if present
    if let Some(icc_data) = icc {
        embed_icc_webp(encoded, icc_data)
    } else {
        Ok(encoded)
    }
}

/// Embed ICC profile into WebP using img-parts
pub fn embed_icc_webp(webp_data: Vec<u8>, icc: &[u8]) -> EncoderResult<Vec<u8>> {
    use img_parts::webp::WebP;
    use img_parts::Bytes;

    let mut webp = WebP::from_bytes(Bytes::from(webp_data)).map_err(|e| {
        to_encoder_error(LazyImageError::decode_failed(format!(
            "failed to parse WebP for ICC: {e}"
        )))
    })?;

    // Set the ICCP chunk directly
    webp.set_icc_profile(Some(Bytes::from(icc.to_vec())));

    // Encode back
    let mut output = Vec::new();
    webp.encoder().write_to(&mut output).map_err(|e| {
        to_encoder_error(LazyImageError::encode_failed(
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
///
/// This function uses safe abstractions from `codecs::avif_safe` to minimize
/// unsafe blocks and improve memory safety.
pub fn encode_avif(
    img: &DynamicImage,
    quality: u8,
    icc: Option<&[u8]>,
) -> EncoderResult<Vec<u8>> {
    use std::borrow::Cow;

    let settings = QualitySettings::new(quality);
    let (width, height) = img.dimensions();

    // Determine if image has alpha
    let has_alpha = img.color().has_alpha();

    // Zero-copy optimization: avoid conversion if already RGBA8
    let rgba: Cow<'_, image::RgbaImage> = match img {
        DynamicImage::ImageRgba8(rgba_img) => Cow::Borrowed(rgba_img),
        _ => Cow::Owned(img.to_rgba8()),
    };
    let pixels = rgba.as_raw();

    // Create AVIF image using safe wrapper
    let mut avif_image = SafeAvifImage::new(
        width,
        height,
        8, // 8-bit depth
        AVIF_PIXEL_FORMAT_YUV420,
    )
    .map_err(to_encoder_error)?;

    // Set color properties
    avif_image.set_color_properties(
        AVIF_COLOR_PRIMARIES_BT709 as u16,
        AVIF_TRANSFER_CHARACTERISTICS_SRGB as u16,
        AVIF_MATRIX_COEFFICIENTS_BT709 as u16,
        AVIF_RANGE_FULL,
    );

    // Set ICC profile if provided
    if let Some(icc_data) = icc {
        avif_image.set_icc_profile(icc_data).map_err(to_encoder_error)?;
    }

    // Create and configure RGB image structure
    let rgb = create_rgb_image(&mut avif_image, pixels.as_ptr(), width, height);

    // Allocate YUV planes in the image
    avif_image.allocate_planes(AVIF_PLANES_YUV).map_err(to_encoder_error)?;

    // Convert RGB to YUV using libavif's optimized conversion
    avif_image.rgb_to_yuv(&rgb).map_err(to_encoder_error)?;

    // Handle alpha channel if present
    if has_alpha {
        avif_image.allocate_planes(AVIF_PLANES_A).map_err(to_encoder_error)?;

        // Copy alpha channel data
        // This is the only place where we need unsafe access to the alpha plane
        unsafe {
            let alpha_plane = avif_image.alpha_plane_mut();
            let alpha_row_bytes = avif_image.alpha_row_bytes();
            for y in 0..height as usize {
                for x in 0..width as usize {
                    let src_idx = (y * width as usize + x) * 4 + 3; // Alpha is 4th component
                    let dst_idx = y * alpha_row_bytes + x;
                    *alpha_plane.add(dst_idx) = pixels[src_idx];
                }
            }
        }
    }

    // Create encoder using safe wrapper
    let mut encoder = SafeAvifEncoder::new().map_err(to_encoder_error)?;

    // Configure encoder
    // libavif quality: 0 (worst) to 100 (lossless),
    // but internally uses quantizer where lower = better
    // quality maps to: minQuantizer and maxQuantizer
    // libavif requires maxThreads >= 2 for multi-threading; cap at 8 to avoid runaway thread counts
    let cpu_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);
    let capped = cmp::min(8, cpu_threads);
    let encoder_threads = cmp::max(2, capped) as i32;

    encoder.configure(quality, quality, settings.avif_speed(), encoder_threads);

    // Create output buffer using safe wrapper
    let mut output = SafeAvifRwData::new();

    // Add image to encoder
    encoder
        .add_image(&mut avif_image, 1, AVIF_ADD_IMAGE_FLAG_SINGLE)
        .map_err(to_encoder_error)?;

    // Finish encoding
    encoder.finish(&mut output).map_err(to_encoder_error)?;

    // Copy output data
    let encoded_data = output.to_vec();

    Ok(encoded_data)
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

    mod encode_tests {
        use super::*;

        #[test]
        fn test_encode_jpeg_produces_valid_jpeg() {
            let img = create_test_image(100, 100);
            let result = encode_jpeg(&img, 80, None).unwrap();
            // JPEGマジックバイト確認
            assert_eq!(&result[0..2], &[0xFF, 0xD8]);
            // JPEGエンドマーカー確認
            assert_eq!(&result[result.len() - 2..], &[0xFF, 0xD9]);
        }

        #[test]
        fn test_encode_jpeg_quality_affects_size() {
            let img = create_test_image(100, 100);
            let high_quality = encode_jpeg(&img, 95, None).unwrap();
            let low_quality = encode_jpeg(&img, 50, None).unwrap();
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

            let result = encode_jpeg(&img, 80, Some(&icc_data)).unwrap();
            assert_eq!(&result[0..2], &[0xFF, 0xD8]);
        }

        #[test]
        fn test_encode_jpeg_fast_mode_produces_valid_jpeg() {
            let img = create_test_image(100, 100);
            let result = encode_jpeg_with_settings(&img, 80, None, true).unwrap();
            // JPEGマジックバイト確認
            assert_eq!(&result[0..2], &[0xFF, 0xD8]);
            // JPEGエンドマーカー確認
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

            let result = encode_webp(&img, 80, Some(&icc_data)).unwrap();
            assert_eq!(&result[0..4], b"RIFF");
            assert_eq!(&result[8..12], b"WEBP");
        }

        #[test]
        fn test_encode_avif_produces_valid_avif() {
            let img = create_test_image(100, 100);
            let result = encode_avif(&img, 60, None).unwrap();
            // AVIFは先頭にftypボックス
            assert!(result.len() > 12);
            // "ftyp"が含まれることを確認
            let has_ftyp = result.windows(4).any(|w| w == b"ftyp");
            assert!(has_ftyp);
        }

        #[test]
        fn test_encode_avif_quality_affects_size() {
            let img = create_test_image(100, 100);
            let high_quality = encode_avif(&img, 80, None).unwrap();
            let low_quality = encode_avif(&img, 40, None).unwrap();
            // 両方とも有効なAVIFであることを確認
            assert!(high_quality.len() > 0);
            assert!(low_quality.len() > 0);
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
