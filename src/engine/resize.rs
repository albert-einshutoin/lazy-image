// src/engine/resize.rs
//
// Resize operations: dimension calculations, fast_image_resize dispatch,
// format-specific resize implementations, and fallback logic.

use crate::error::LazyImageError;
use fast_image_resize::{self as fir, ImageBufferError, MulDiv, PixelType, ResizeOptions};
use image::{imageops::FilterType, DynamicImage, RgbImage, RgbaImage};

#[derive(Debug)]
pub struct ResizeError {
    pub source_dims: (u32, u32),
    pub target_dims: (u32, u32),
    pub reason: String,
}

impl ResizeError {
    pub fn new(
        source_dims: (u32, u32),
        target_dims: (u32, u32),
        reason: impl Into<String>,
    ) -> Self {
        Self {
            source_dims,
            target_dims,
            reason: reason.into(),
        }
    }

    pub fn into_lazy_image_error(self) -> LazyImageError {
        LazyImageError::resize_failed(self.source_dims, self.target_dims, self.reason)
    }
}

/// Calculate resize dimensions maintaining aspect ratio (fit = inside semantics)
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

pub(crate) fn validate_resize_dimensions(width: u32, height: u32) -> Result<(), LazyImageError> {
    if width == 0 || height == 0 {
        return Err(LazyImageError::invalid_resize_dimensions(
            Some(width),
            Some(height),
        ));
    }
    Ok(())
}

pub(crate) fn calc_cover_resize_dimensions(
    orig_w: u32,
    orig_h: u32,
    target_w: u32,
    target_h: u32,
) -> (u32, u32) {
    if orig_w == 0 || orig_h == 0 {
        return (target_w.max(1), target_h.max(1));
    }
    let scale_w = target_w as f64 / orig_w as f64;
    let scale_h = target_h as f64 / orig_h as f64;
    let scale = scale_w.max(scale_h);
    let resize_w = ((orig_w as f64 * scale).ceil() as u32).max(1);
    let resize_h = ((orig_h as f64 * scale).ceil() as u32).max(1);
    (resize_w, resize_h)
}

pub(crate) fn crop_to_dimensions(img: DynamicImage, target_w: u32, target_h: u32) -> DynamicImage {
    let crop_width = target_w.min(img.width()).max(1);
    let crop_height = target_h.min(img.height()).max(1);
    let crop_x = if img.width() > crop_width {
        (img.width() - crop_width) / 2
    } else {
        0
    };
    let crop_y = if img.height() > crop_height {
        (img.height() - crop_height) / 2
    } else {
        0
    };
    img.crop_imm(crop_x, crop_y, crop_width, crop_height)
}

pub(crate) fn default_resize_options() -> ResizeOptions {
    ResizeOptions::new().resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::Lanczos3))
}

/// Fast resize with owned DynamicImage (zero-copy for RGB/RGBA)
/// Returns Ok(resized) on success, Err(resize_error) on failure
pub fn fast_resize_owned(
    img: DynamicImage,
    dst_width: u32,
    dst_height: u32,
) -> std::result::Result<DynamicImage, ResizeError> {
    fast_resize_owned_impl(img, dst_width, dst_height, default_resize_options())
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

    // Select pixel layout without forcing RGBA when not needed.
    // Use as_raw().to_vec() to copy only the raw pixel bytes, avoiding the
    // overhead of cloning the entire ImageBuffer (which includes width, height,
    // and phantom-type metadata).  For non-RGB8/RGBA8 formats we must convert
    // to RGBA8 first, which already produces an owned buffer.
    let (pixel_type, src_pixels): (PixelType, Vec<u8>) = match img {
        DynamicImage::ImageRgb8(rgb) => (PixelType::U8x3, rgb.as_raw().to_vec()),
        DynamicImage::ImageRgba8(rgba) => (PixelType::U8x4, rgba.as_raw().to_vec()),
        _ => {
            let rgba = img.to_rgba8();
            (PixelType::U8x4, rgba.into_raw())
        }
    };

    fast_resize_internal_with_options(
        src_width,
        src_height,
        src_pixels,
        pixel_type,
        dst_width,
        dst_height,
        default_resize_options(),
    )
}

/// Internal resize implementation (shared by both owned and reference versions)
pub fn fast_resize_internal_with_options(
    src_width: u32,
    src_height: u32,
    src_pixels: Vec<u8>,
    pixel_type: PixelType,
    dst_width: u32,
    dst_height: u32,
    options: ResizeOptions,
) -> std::result::Result<DynamicImage, String> {
    fast_resize_internal_impl(
        src_width, src_height, src_pixels, pixel_type, dst_width, dst_height, options,
    )
}

/// Backward-compatible helper preserving the legacy signature without options.
pub fn fast_resize_internal(
    src_width: u32,
    src_height: u32,
    src_pixels: Vec<u8>,
    pixel_type: PixelType,
    dst_width: u32,
    dst_height: u32,
) -> std::result::Result<DynamicImage, String> {
    fast_resize_internal_with_options(
        src_width,
        src_height,
        src_pixels,
        pixel_type,
        dst_width,
        dst_height,
        default_resize_options(),
    )
}

pub(crate) fn fast_resize_owned_impl(
    img: DynamicImage,
    dst_width: u32,
    dst_height: u32,
    options: ResizeOptions,
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
        src_width, src_height, src_pixels, pixel_type, dst_width, dst_height, options,
    )
    .map_err(|reason| ResizeError::new((src_width, src_height), (dst_width, dst_height), reason))
}

/// Decide whether alpha premultiplication is required for a given pixel layout.
#[inline]
pub(crate) fn requires_premultiply(pixel_type: PixelType) -> bool {
    matches!(pixel_type, PixelType::U8x4)
}

/// Check if an RGBA image is fully opaque (all alpha values are 255)
/// For RGB images, always returns true (no alpha channel)
///
/// Only checks images ≥1MP - for smaller images, the check overhead exceeds
/// the premultiply cost (SIMD premultiply is very fast for small images)
pub(crate) fn is_fully_opaque(
    image: &fir::images::Image,
    pixel_type: PixelType,
    width: u32,
    height: u32,
) -> bool {
    if pixel_type != PixelType::U8x4 {
        return true; // RGB images have no alpha channel
    }

    // Size threshold: Only check large images (≥1MP)
    // For small images, premultiply is cheap (SIMD-optimized), skip the scan
    const THRESHOLD_PIXELS: u32 = 1_000_000; // 1 megapixel
    if (width as u64).saturating_mul(height as u64) < THRESHOLD_PIXELS as u64 {
        return false; // Assume not opaque, do premultiply (it's fast anyway)
    }

    // Check every 4th byte (alpha channel) in RGBA data
    // Conservative: if any alpha < 255, return false
    let buffer = image.buffer();
    buffer.iter().skip(3).step_by(4).all(|&alpha| alpha == 255)
}

pub(crate) fn copy_pixels_to_aligned_image(
    width: u32,
    height: u32,
    pixel_type: PixelType,
    src_pixels: &[u8],
    required_bytes: usize,
) -> std::result::Result<fir::images::Image<'static>, String> {
    let mut aligned_image = fir::images::Image::new(width, height, pixel_type);
    let aligned_buffer = aligned_image.buffer_mut();
    if aligned_buffer.len() != required_bytes {
        return Err(format!(
            "fir alignment fallback buffer mismatch. expected {required_bytes} bytes, got {} bytes",
            aligned_buffer.len()
        ));
    }
    aligned_buffer.copy_from_slice(&src_pixels[..required_bytes]);
    Ok(aligned_image)
}

pub(crate) fn resize_with_image_crate_fallback(
    src_pixels: &[u8],
    src_width: u32,
    src_height: u32,
    pixel_type: PixelType,
    dst_width: u32,
    dst_height: u32,
) -> std::result::Result<DynamicImage, String> {
    let filter = FilterType::Lanczos3;
    match pixel_type {
        PixelType::U8x3 => {
            let rgb = RgbImage::from_raw(src_width, src_height, src_pixels.to_vec())
                .ok_or_else(|| "failed to build rgb image for fallback resize".to_string())?;
            Ok(DynamicImage::ImageRgb8(image::imageops::resize(
                &rgb, dst_width, dst_height, filter,
            )))
        }
        PixelType::U8x4 => {
            let rgba = RgbaImage::from_raw(src_width, src_height, src_pixels.to_vec())
                .ok_or_else(|| "failed to build rgba image for fallback resize".to_string())?;
            Ok(DynamicImage::ImageRgba8(image::imageops::resize(
                &rgba, dst_width, dst_height, filter,
            )))
        }
        _ => Err("fallback resize supports only U8x3/U8x4 pixel types".to_string()),
    }
}

pub(crate) fn fast_resize_internal_impl(
    src_width: u32,
    src_height: u32,
    mut src_pixels: Vec<u8>,
    pixel_type: PixelType,
    dst_width: u32,
    dst_height: u32,
    options: ResizeOptions,
) -> std::result::Result<DynamicImage, String> {
    let pixel_count = (src_width as usize)
        .checked_mul(src_height as usize)
        .ok_or_else(|| "image dimensions overflow during resize".to_string())?;
    let required_bytes = pixel_count
        .checked_mul(pixel_type.size())
        .ok_or_else(|| "image buffer size overflow during resize".to_string())?;

    if src_pixels.len() < required_bytes {
        return Err(format!(
            "fir source image invalid buffer size. expected {required_bytes} bytes, got {} bytes",
            src_pixels.len()
        ));
    }

    let primary_result = match fir::images::Image::from_slice_u8(
        src_width,
        src_height,
        src_pixels.as_mut_slice(),
        pixel_type,
    ) {
        Ok(src_image) => {
            resize_with_source_image(src_image, pixel_type, dst_width, dst_height, options)
        }
        Err(ImageBufferError::InvalidBufferAlignment) => {
            let aligned_image = copy_pixels_to_aligned_image(
                src_width,
                src_height,
                pixel_type,
                &src_pixels,
                required_bytes,
            )?;
            resize_with_source_image(aligned_image, pixel_type, dst_width, dst_height, options)
        }
        Err(other) => Err(format!("fir source image error: {other:?}")),
    };

    match primary_result {
        Ok(img) => Ok(img),
        Err(err) => resize_with_image_crate_fallback(
            &src_pixels,
            src_width,
            src_height,
            pixel_type,
            dst_width,
            dst_height,
        )
        .map_err(|fallback_err| format!("{err}; image crate fallback failed: {fallback_err}")),
    }
}

fn resize_with_source_image<'a>(
    mut src_image: fir::images::Image<'a>,
    pixel_type: PixelType,
    dst_width: u32,
    dst_height: u32,
    options: ResizeOptions,
) -> std::result::Result<DynamicImage, String> {
    let mut dst_image = fir::images::Image::new(dst_width, dst_height, pixel_type);

    // Optimization: Skip premultiply/unpremultiply for fully opaque images
    // Check if all alpha values are 255 (fully opaque)
    // Only checks large images (≥1MP) where the benefit outweighs the scan cost
    let src_width = src_image.width();
    let src_height = src_image.height();
    let needs_premultiply = requires_premultiply(pixel_type)
        && !is_fully_opaque(&src_image, pixel_type, src_width, src_height);

    let mul_div = MulDiv::default();
    if needs_premultiply {
        mul_div
            .multiply_alpha_inplace(&mut src_image)
            .map_err(|e| format!("failed to premultiply alpha: {e}"))?;
    }

    let mut resizer = fir::Resizer::new();
    resizer
        .resize(&src_image, &mut dst_image, &options)
        .map_err(|e| format!("fir resize error: {e:?}"))?;

    if needs_premultiply {
        mul_div
            .divide_alpha_inplace(&mut dst_image)
            .map_err(|e| format!("failed to unpremultiply alpha: {e}"))?;
    }

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
mod tests {
    use super::*;
    use image::{DynamicImage, GenericImageView, RgbImage, RgbaImage};

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
            assert_eq!(h, 250);
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
            let (w, h) = calc_resize_dimensions(101, 51, Some(50), None);
            assert_eq!(w, 50);
            assert_eq!(h, 25);
        }

        #[test]
        fn test_aspect_ratio_preservation_wide() {
            let (w, h) = calc_resize_dimensions(2000, 1000, Some(1000), None);
            assert_eq!(w, 1000);
            assert_eq!(h, 500);
        }

        #[test]
        fn test_aspect_ratio_preservation_tall() {
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
            let (w, h) = calc_resize_dimensions(6000, 4000, Some(800), Some(600));
            assert_eq!(w, 800);
            assert_eq!(h, 533);
        }

        #[test]
        fn test_both_dimensions_tall_image_fits_inside() {
            let (w, h) = calc_resize_dimensions(4000, 6000, Some(800), Some(600));
            assert_eq!(w, 400);
            assert_eq!(h, 600);
        }

        #[test]
        fn test_both_dimensions_same_aspect_ratio() {
            let (w, h) = calc_resize_dimensions(1000, 500, Some(800), Some(400));
            assert_eq!((w, h), (800, 400));
        }
    }

    mod resize_fallback_tests {
        use super::*;

        fn generate_pixels(count: usize) -> Vec<u8> {
            (0..count).map(|i| (i % 251) as u8).collect()
        }

        #[test]
        fn image_crate_fallback_resizes_rgb() {
            let src_width = 8;
            let src_height = 4;
            let pixel_type = PixelType::U8x3;
            let src_pixels = generate_pixels((src_width * src_height) as usize * pixel_type.size());

            let resized = resize_with_image_crate_fallback(
                &src_pixels,
                src_width,
                src_height,
                pixel_type,
                4,
                2,
            )
            .expect("fallback resize should succeed for RGB");

            assert_eq!(resized.dimensions(), (4, 2));
            assert!(matches!(resized, DynamicImage::ImageRgb8(_)));
        }

        #[test]
        fn image_crate_fallback_resizes_rgba() {
            let src_width = 6;
            let src_height = 3;
            let pixel_type = PixelType::U8x4;
            let src_pixels = generate_pixels((src_width * src_height) as usize * pixel_type.size());

            let resized = resize_with_image_crate_fallback(
                &src_pixels,
                src_width,
                src_height,
                pixel_type,
                3,
                2,
            )
            .expect("fallback resize should succeed for RGBA");

            assert_eq!(resized.dimensions(), (3, 2));
            assert!(matches!(resized, DynamicImage::ImageRgba8(_)));
        }
    }

    mod fast_resize_tests {
        use super::*;

        #[test]
        fn test_fast_resize_downscale() {
            let img = create_test_image(200, 200);
            let result = fast_resize(&img, 100, 100);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 100));
        }

        #[test]
        fn test_fast_resize_upscale() {
            let img = create_test_image(50, 50);
            let result = fast_resize(&img, 100, 100);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 100));
        }

        #[test]
        fn test_fast_resize_aspect_ratio_change() {
            let img = create_test_image(200, 100);
            let result = fast_resize(&img, 100, 200);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 200));
        }

        #[test]
        fn test_fast_resize_invalid_dimensions() {
            let img = create_test_image(100, 100);
            let result = fast_resize(&img, 0, 100);
            assert!(result.is_err());
        }

        #[test]
        fn test_fast_resize_same_size() {
            let img = create_test_image(100, 100);
            let result = fast_resize(&img, 100, 100);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (100, 100));
        }

        #[test]
        fn test_fast_resize_rgba() {
            let img = create_test_image_rgba(100, 100);
            let result = fast_resize(&img, 50, 50);
            assert!(result.is_ok());
            let resized = result.unwrap();
            assert_eq!(resized.dimensions(), (50, 50));
        }

        #[test]
        fn test_requires_premultiply_only_for_rgba() {
            assert!(requires_premultiply(PixelType::U8x4));
            assert!(!requires_premultiply(PixelType::U8x3));
        }

        #[test]
        fn test_fast_resize_rgba_respects_transparency_when_downscaling() {
            let mut img = DynamicImage::ImageRgba8(RgbaImage::new(2, 1));
            {
                let buf = img.as_mut_rgba8().unwrap();
                buf.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
                buf.put_pixel(1, 0, image::Rgba([0, 0, 255, 0]));
            }

            let resized = fast_resize(&img, 1, 1).expect("resize should succeed");
            let resized_rgba = resized.to_rgba8();
            let pixel = resized_rgba.get_pixel(0, 0);
            assert!(
                pixel[0] > 200,
                "red channel should dominate, got {}",
                pixel[0]
            );
            assert!(
                pixel[2] < 30,
                "blue channel should be minimal, got {}",
                pixel[2]
            );
            assert!(
                pixel[3] > 100,
                "alpha should remain non-zero, got {}",
                pixel[3]
            );
        }

        #[test]
        fn test_is_fully_opaque_skips_small_image_scan() {
            let width = 512;
            let height = 512;
            let mut pixels = vec![255u8; (width * height * 4) as usize];
            let image = fir::images::Image::from_slice_u8(
                width,
                height,
                pixels.as_mut_slice(),
                PixelType::U8x4,
            )
            .expect("valid RGBA image");

            assert!(
                !is_fully_opaque(&image, PixelType::U8x4, width, height),
                "small RGBA images should skip opacity scan"
            );
        }

        #[test]
        fn test_is_fully_opaque_scans_large_image() {
            let width = 1000;
            let height = 1000;
            let mut pixels = vec![255u8; (width * height * 4) as usize];
            let image = fir::images::Image::from_slice_u8(
                width,
                height,
                pixels.as_mut_slice(),
                PixelType::U8x4,
            )
            .expect("valid RGBA image");

            assert!(is_fully_opaque(&image, PixelType::U8x4, width, height));
        }

        #[test]
        fn test_is_fully_opaque_uses_wide_multiplication_for_threshold() {
            let mut pixels = vec![255u8; 4];
            let image =
                fir::images::Image::from_slice_u8(1, 1, pixels.as_mut_slice(), PixelType::U8x4)
                    .expect("valid RGBA image");

            assert!(
                is_fully_opaque(&image, PixelType::U8x4, u32::MAX, u32::MAX),
                "overflow-safe pixel count should not force small-image fast path"
            );
        }
    }

    #[test]
    fn test_copy_pixels_to_aligned_image_preserves_data() {
        let width = 2;
        let height = 2;
        let mut src_pixels = vec![0u8; (width * height * 4) as usize];
        for (idx, byte) in src_pixels.iter_mut().enumerate() {
            *byte = idx as u8;
        }

        let image = copy_pixels_to_aligned_image(
            width,
            height,
            PixelType::U8x4,
            &src_pixels,
            src_pixels.len(),
        )
        .expect("should copy into aligned buffer");

        assert_eq!(image.buffer(), src_pixels.as_slice());
    }

    #[test]
    fn test_fast_resize_internal_impl_errors_on_short_buffer() {
        let res = fast_resize_internal_impl(
            4,
            4,
            vec![0u8; 10],
            PixelType::U8x3,
            2,
            2,
            default_resize_options(),
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_fast_resize_uses_rayon_pool() {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .expect("failed to build test pool");

        pool.install(|| {
            let src_width = 256;
            let src_height = 256;
            let dst_width = 64;
            let dst_height = 64;

            let src_pixels: Vec<u8> = (0..(src_width * src_height))
                .flat_map(|i| {
                    let v = (i as u8).wrapping_mul(13);
                    [v, v, v, 255]
                })
                .collect();

            let result = fast_resize_internal_impl(
                src_width,
                src_height,
                src_pixels,
                PixelType::U8x4,
                dst_width,
                dst_height,
                default_resize_options(),
            );

            assert!(result.is_ok(), "resize failed inside rayon pool");
            let img = result.unwrap();
            assert_eq!(img.width(), dst_width);
            assert_eq!(img.height(), dst_height);
        });
    }
}
