#![no_main]

//! Fuzz target for image decoding paths in lazy-image.
//! Tests lazy-image's specific decoders: mozjpeg (JPEG) and image crate wrapper (PNG/WebP).
//!
//! To stay under CI's 2GB RSS limit (ASan + corpus), we only run full decode on inputs that
//! pass ensure_dimensions_safe (fuzz limits: 1024/1M). Large images are rejected before any
//! heavy allocation, so we never OOM from a single input.

use lazy_image::engine::{
    decode_jpeg_mozjpeg, decode_with_image_crate, detect_format, ensure_dimensions_safe,
};
use lazy_image::inspect_header_from_bytes;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // Test 1: lazy-image's header inspection (fast path, no full decode)
    let _ = inspect_header_from_bytes(data);

    // Reject inputs that would exceed fuzz decode budget (1024/1M). This ensures we never
    // run a full decode on huge dimensions, avoiding OOM regardless of decoder internals.
    if ensure_dimensions_safe(data).is_err() {
        return;
    }

    // Test 2: lazy-image's mozjpeg decoder (JPEG-specific path)
    if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8 {
        let _ = decode_jpeg_mozjpeg(data);
    }

    // Test 3: lazy-image's image crate wrapper (PNG/WebP/other formats)
    if detect_format(data).is_some() {
        let _ = decode_with_image_crate(data);
    }
});
