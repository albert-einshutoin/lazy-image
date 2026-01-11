// src/engine/stress.rs
//
// Stress test utilities for fuzzing and performance testing.
// This module is independent of NAPI and can be used with --no-default-features --features stress.

use crate::convert_result;
use crate::engine::common::EngineResult;
use crate::engine::decoder::decode_jpeg_mozjpeg;
use crate::engine::encoder::{encode_avif, encode_jpeg, encode_png, encode_webp};
use crate::engine::pipeline::apply_ops;
use crate::ops::{Operation, OutputFormat};
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
        convert_result!(decode_jpeg_mozjpeg(data))
    } else {
        // Other formats - use image crate
        image::load_from_memory(data).map_err(|e| {
            crate::error::LazyImageError::decode_failed(format!("decode failed: {e}"))
        })?
    };

    // Apply operations and encode in each format
    for format in formats.into_iter() {
        let processed = convert_result!(apply_ops(Cow::Borrowed(&img), &operations));

        // Encode to the target format
        let _encoded = match format {
            OutputFormat::Jpeg { quality } => {
                convert_result!(encode_jpeg(&processed, quality, None))
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
