#![no_main]

//! Fuzz target for image decoding.
//! Tests JPEG, PNG, WebP decoding paths for crashes and memory issues.

use image::ImageReader;
use libfuzzer_sys::fuzz_target;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // Test decoding with image crate (same path as lazy-image)
    let cursor = Cursor::new(data);
    if let Ok(reader) = ImageReader::new(cursor).with_guessed_format() {
        let _ = reader.decode();
    }
});
