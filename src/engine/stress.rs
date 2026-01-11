// src/engine/stress.rs
//
// Stress test utilities for fuzzing and performance testing.
// This module is independent of NAPI and can be used with --no-default-features --features stress.

use crate::engine::decoder::decode_jpeg_mozjpeg;
use crate::engine::encoder::{encode_avif, encode_jpeg, encode_png, encode_webp};
use crate::engine::pipeline::apply_ops;
use crate::error::LazyImageError;
use crate::ops::{Operation, OutputFormat};
use image::DynamicImage;
use std::borrow::Cow;

// Type alias for Result - use standard Result when napi is disabled
type EngineResult<T> = std::result::Result<T, LazyImageError>;

/// Run a single stress test iteration.
///
/// This function processes an image through multiple operations and formats
/// to test the pipeline for memory leaks and correctness.
///
/// # Arguments
/// * `data` - Raw image bytes (JPEG, PNG, WebP, etc.)
///
/// # Returns
/// * `Ok(())` if processing succeeds
/// * `Err(LazyImageError)` if any step fails
#[cfg(feature = "stress")]
pub fn run_stress_iteration(data: &[u8]) -> EngineResult<()> {
    let operations: Vec<Operation> = vec![
        Operation::Resize {
            width: Some(1200),
            height: Some(800),
        },
        Operation::Rotate { degrees: 90 },
        Operation::Brightness { value: 12 },
        Operation::Contrast { value: -6 },
        Operation::Grayscale,
    ];

    let formats = [
        OutputFormat::Jpeg { quality: 82 },
        OutputFormat::Png,
        OutputFormat::WebP { quality: 74 },
        OutputFormat::Avif { quality: 60 },
    ];

    // Decode the image once
    let img = if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8 {
        // JPEG - use mozjpeg for speed
        // decode_jpeg_mozjpeg returns DecoderResult which is napi::Result when napi is enabled
        // or std::result::Result when napi is disabled. We need to handle both cases.
        #[cfg(feature = "napi")]
        {
            decode_jpeg_mozjpeg(data).map_err(|e| {
                LazyImageError::decode_failed(format!("mozjpeg decode failed: {}", e.to_string()))
            })?
        }
        #[cfg(not(feature = "napi"))]
        {
            decode_jpeg_mozjpeg(data)?
        }
    } else {
        // Other formats - use image crate
        image::load_from_memory(data).map_err(|e| {
            LazyImageError::decode_failed(format!("decode failed: {e}"))
        })?
    };

    // Apply operations and encode in each format
    for format in formats.into_iter() {
        // apply_ops returns PipelineResult which is napi::Result when napi is enabled
        // or std::result::Result when napi is disabled. We need to handle both cases.
        #[cfg(feature = "napi")]
        let processed = {
            apply_ops(Cow::Borrowed(&img), &operations).map_err(|e| {
                LazyImageError::resize_failed(
                    (img.width(), img.height()),
                    (1200, 800),
                    e.to_string(),
                )
            })?
        };
        #[cfg(not(feature = "napi"))]
        let processed = {
            apply_ops(Cow::Borrowed(&img), &operations)?
        };

        // Encode to the target format
        // Encoder functions return EncoderResult which is napi::Result when napi is enabled
        // or std::result::Result when napi is disabled. We need to handle both cases.
        let _encoded = match format {
            OutputFormat::Jpeg { quality } => {
                #[cfg(feature = "napi")]
                {
                    encode_jpeg(&processed, quality, None).map_err(|e| {
                        LazyImageError::encode_failed(Cow::Borrowed("jpeg"), e.to_string())
                    })?
                }
                #[cfg(not(feature = "napi"))]
                {
                    encode_jpeg(&processed, quality, None)?
                }
            }
            OutputFormat::Png => {
                #[cfg(feature = "napi")]
                {
                    encode_png(&processed, None).map_err(|e| {
                        LazyImageError::encode_failed(Cow::Borrowed("png"), e.to_string())
                    })?
                }
                #[cfg(not(feature = "napi"))]
                {
                    encode_png(&processed, None)?
                }
            }
            OutputFormat::WebP { quality } => {
                #[cfg(feature = "napi")]
                {
                    encode_webp(&processed, quality, None).map_err(|e| {
                        LazyImageError::encode_failed(Cow::Borrowed("webp"), e.to_string())
                    })?
                }
                #[cfg(not(feature = "napi"))]
                {
                    encode_webp(&processed, quality, None)?
                }
            }
            OutputFormat::Avif { quality } => {
                #[cfg(feature = "napi")]
                {
                    encode_avif(&processed, quality, None).map_err(|e| {
                        LazyImageError::encode_failed(Cow::Borrowed("avif"), e.to_string())
                    })?
                }
                #[cfg(not(feature = "napi"))]
                {
                    encode_avif(&processed, quality, None)?
                }
            }
        };

        // stress harness only needs to ensure the pipeline runs without leaking; drop the result
    }

    Ok(())
}
