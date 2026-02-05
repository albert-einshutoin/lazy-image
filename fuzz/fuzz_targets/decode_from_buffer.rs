#![no_main]

//! Fuzz target for image decoding paths in lazy-image.
//! Tests lazy-image's specific decoders: mozjpeg (JPEG) and image crate wrapper (PNG/WebP).

use lazy_image::engine::{decode_jpeg_mozjpeg, decode_with_image_crate, detect_format};
use lazy_image::inspect_header_from_bytes;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // Test 1: lazy-image's header inspection (fast path)
    let _ = inspect_header_from_bytes(data);

    // Test 2: lazy-image's mozjpeg decoder (JPEG-specific path)
    // This is lazy-image's custom decoder using mozjpeg/libjpeg-turbo
    if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8 {
        let _ = decode_jpeg_mozjpeg(data);
    }

    // Test 3: lazy-image's image crate wrapper (PNG/WebP/other formats)
    // This tests the panic-safe wrapper around the image crate. Skip unknown formats
    // to avoid unnecessary OOM risk from random bytes.
    if detect_format(data).is_some() {
        let _ = decode_with_image_crate(data);
    }
});
