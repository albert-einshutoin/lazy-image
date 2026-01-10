// src/engine/decoder.rs
//
// Decoder operations: JPEG (mozjpeg), PNG, WebP, etc.

use crate::engine::io::Source;
use crate::error::LazyImageError;
use image::{DynamicImage, GenericImageView, RgbImage};
use mozjpeg::Decompress;
use std::panic;

use crate::engine::MAX_DIMENSION;

// Type alias for Result - use napi::Result when napi is enabled, otherwise use standard Result
#[cfg(feature = "napi")]
use napi::bindgen_prelude::*;
#[cfg(feature = "napi")]
type DecoderResult<T> = Result<T>;
#[cfg(not(feature = "napi"))]
type DecoderResult<T> = std::result::Result<T, LazyImageError>;

// Helper function to convert LazyImageError to the appropriate error type
#[cfg(feature = "napi")]
fn to_decoder_error(err: LazyImageError) -> napi::Error {
    napi::Error::from(err)
}

#[cfg(not(feature = "napi"))]
fn to_decoder_error(err: LazyImageError) -> LazyImageError {
    err
}

/// Decode image from source bytes
pub fn decode(source: &Source) -> DecoderResult<DynamicImage> {
    let bytes = match source {
        Source::Memory(data) => data.as_slice(),
        Source::Path(_) => {
            return Err(to_decoder_error(LazyImageError::internal_panic(
                "decode called with Path source - must load bytes first",
            )));
        }
    };

    // Check magic bytes for JPEG (0xFF 0xD8)
    let img = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        // JPEG detected - use mozjpeg for TURBO speed
        decode_jpeg_mozjpeg(bytes)?
    } else {
        // PNG, WebP, etc - use image crate
        image::load_from_memory(bytes).map_err(|e| {
            to_decoder_error(LazyImageError::decode_failed(format!("decode failed: {e}")))
        })?
    };

    // Security check: reject decompression bombs
    let (w, h) = img.dimensions();
    check_dimensions(w, h)?;

    Ok(img)
}

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
        Ok(Err(e)) => Err(to_decoder_error(LazyImageError::decode_failed(e))),
        Err(_) => Err(to_decoder_error(LazyImageError::internal_panic(
            "mozjpeg panicked during decode",
        ))),
    }
}

/// Check if image dimensions are within safe limits.
/// Returns an error if the image is too large (potential decompression bomb).
#[cfg(feature = "napi")]
pub fn check_dimensions(width: u32, height: u32) -> Result<()> {
    use super::MAX_PIXELS;
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
