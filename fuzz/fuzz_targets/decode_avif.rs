#![no_main]

//! Fuzz target for AVIF decoding paths in lazy-image.
//! Exercises the unified decoder on inputs that look like AVIF (ISOBMFF with ftyp box).

use lazy_image::engine::{decode_image, ensure_dimensions_safe};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Only process data that could be AVIF (ISOBMFF with ftyp box)
    if data.len() < 12 {
        return;
    }
    if &data[4..8] != b"ftyp" {
        return;
    }

    // Reject inputs that would exceed fuzz decode budget
    if ensure_dimensions_safe(data).is_err() {
        return;
    }

    // Exercise the AVIF decode path through the unified decoder
    let _ = decode_image(data);
});
