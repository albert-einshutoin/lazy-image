// src/engine/decoder.rs
//
// Decoder operations: JPEG (mozjpeg), PNG, WebP, etc.

use crate::error::LazyImageError;
#[cfg(test)]
use image::GenericImageView;
use image::{DynamicImage, ImageReader, RgbImage};
use mozjpeg::Decompress;
use std::io::Cursor;
use std::panic;

use crate::engine::MAX_DIMENSION;

// Type alias for Result - always use LazyImageError to preserve error taxonomy
// This ensures that decode errors are properly classified (CodecError, ResourceLimit, etc.)
// rather than being converted to generic InternalBug errors.
type DecoderResult<T> = std::result::Result<T, LazyImageError>;

// decode() function removed - it was unused.
// tasks.rs::EncodeTask::decode() and stress.rs::run_stress_iteration() have their own implementations.

/// Decode JPEG using mozjpeg (backed by libjpeg-turbo)
/// This is SIGNIFICANTLY faster than image crate's pure Rust decoder
pub fn decode_jpeg_mozjpeg(data: &[u8]) -> DecoderResult<DynamicImage> {
    let result = panic::catch_unwind(|| {
        let decompress = Decompress::new_mem(data)
            .map_err(|e| format!("mozjpeg decompress init failed: {e:?}"))?;

        // Get image info
        let mut decompress = decompress
            .rgb()
            .map_err(|e| format!("mozjpeg rgb conversion failed: {e:?}"))?;

        let width = decompress.width();
        let height = decompress.height();

        if width > MAX_DIMENSION as usize || height > MAX_DIMENSION as usize {
            return Err(format!(
                "image dimensions {}x{} exceed max {}",
                width, height, MAX_DIMENSION
            ));
        }
        let width_u32 = width as u32;
        let height_u32 = height as u32;
        if let Err(err) = check_dimensions(width_u32, height_u32) {
            return Err(err.to_string());
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
        let rgb_image = RgbImage::from_raw(width_u32, height_u32, flat_pixels)
            .ok_or_else(|| "mozjpeg: failed to create image from raw data".to_string())?;

        Ok::<DynamicImage, String>(DynamicImage::ImageRgb8(rgb_image))
    });

    match result {
        Ok(Ok(img)) => Ok(img),
        Ok(Err(e)) => Err(LazyImageError::decode_failed(e)),
        Err(_) => Err(LazyImageError::internal_panic(
            "mozjpeg panicked during decode",
        )),
    }
}

/// Check if image dimensions are within safe limits.
/// Returns an error if the image is too large (potential decompression bomb).
pub fn check_dimensions(width: u32, height: u32) -> DecoderResult<()> {
    use super::MAX_PIXELS;
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

/// Inspect encoded bytes and ensure the image dimensions are safe before decoding.
pub fn ensure_dimensions_safe(bytes: &[u8]) -> DecoderResult<()> {
    let cursor = Cursor::new(bytes);
    if let Ok(reader) = ImageReader::new(cursor).with_guessed_format() {
        if let Ok((width, height)) = reader.into_dimensions() {
            return check_dimensions(width, height);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgb, RgbImage};

    fn encode_png(width: u32, height: u32) -> Vec<u8> {
        let img = RgbImage::from_fn(width, height, |_, _| Rgb([0, 0, 0]));
        let mut buffer = Vec::new();
        DynamicImage::ImageRgb8(img)
            .write_to(&mut Cursor::new(&mut buffer), ImageFormat::Png)
            .unwrap();
        buffer
    }

    #[test]
    fn test_ensure_dimensions_safe_allows_small_image() {
        let data = encode_png(64, 64);
        assert!(ensure_dimensions_safe(&data).is_ok());
    }

    #[test]
    fn test_ensure_dimensions_safe_rejects_large_image() {
        let width = crate::engine::MAX_DIMENSION + 1;
        let data = encode_png(width, 1);
        let err = ensure_dimensions_safe(&data).unwrap_err();
        assert!(matches!(err, LazyImageError::DimensionExceedsLimit { .. }));
    }
}
