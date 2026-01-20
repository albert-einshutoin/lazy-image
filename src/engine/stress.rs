// src/engine/stress.rs
//
// Stress test utilities for fuzzing and performance testing.
// This module is independent of NAPI and can be used with --no-default-features --features stress.

#[cfg(feature = "stress")]
use crate::convert_result;
#[cfg(feature = "stress")]
use crate::engine::common::EngineResult;
#[cfg(feature = "stress")]
use crate::engine::decoder::{decode_image, ensure_dimensions_safe};
#[cfg(feature = "stress")]
use crate::engine::encoder::{
    encode_avif, encode_jpeg, encode_jpeg_with_settings, encode_png, encode_webp,
};
#[cfg(feature = "stress")]
use crate::engine::pipeline::apply_ops;
#[cfg(feature = "stress")]
use crate::ops::{Operation, OutputFormat, ResizeFit};
#[cfg(feature = "stress")]
use std::borrow::Cow;

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
            fit: ResizeFit::Inside,
        },
        Operation::Rotate { degrees: 90 },
        Operation::Brightness { value: 12 },
        Operation::Contrast { value: -6 },
        Operation::Grayscale,
    ];

    let formats = [
        OutputFormat::Jpeg {
            quality: 82,
            fast_mode: false,
        },
        OutputFormat::Png,
        OutputFormat::WebP { quality: 74 },
        OutputFormat::Avif { quality: 60 },
    ];

    // Decode the image once
    ensure_dimensions_safe(data)?;
    let (img, _detected_format) = convert_result!(decode_image(data));

    // Apply operations and encode in each format
    for format in formats.into_iter() {
        let processed = convert_result!(apply_ops(Cow::Borrowed(&img), &operations));

        // Encode to the target format
        let _encoded = match format {
            OutputFormat::Jpeg { quality, fast_mode } => {
                convert_result!(encode_jpeg_with_settings(
                    &processed, quality, None, fast_mode
                ))
            }
            OutputFormat::Png => {
                convert_result!(encode_png(&processed, None))
            }
            OutputFormat::WebP { quality } => {
                convert_result!(encode_webp(&processed, quality, None))
            }
            OutputFormat::Avif { quality } => {
                convert_result!(encode_avif(&processed, quality, None))
            }
        };

        // stress harness only needs to ensure the pipeline runs without leaking; drop the result
    }

    Ok(())
}
