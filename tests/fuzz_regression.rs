#![cfg(any(feature = "napi", feature = "fuzzing"))]

//! Regression tests for fuzz-found crashes.
//! Each test should be cheap (single input) and run in the normal test suite.

use lazy_image::engine::{decode_jpeg_mozjpeg, decode_with_image_crate};
use lazy_image::inspect_header_from_bytes;

#[test]
fn fuzz_regression_decode_from_buffer_crash_14aa989e() {
    // Minimal JPEG buffer that previously triggered a fuzz crash in decode_from_buffer target.
    // Stored under tests/data to keep inputs small and versioned.
    let data = include_bytes!("data/crash-decode-from-buffer.bin");

    // Header inspection should never panic.
    inspect_header_from_bytes(data).unwrap();

    // JPEG-specific decoder path.
    decode_jpeg_mozjpeg(data).unwrap();

    // Image crate wrapper path should reject JPEG input gracefully
    // (panic avoidance for zune-jpeg in fuzzing builds).
    assert!(
        decode_with_image_crate(data).is_err(),
        "image crate path should not process JPEG data"
    );
}
