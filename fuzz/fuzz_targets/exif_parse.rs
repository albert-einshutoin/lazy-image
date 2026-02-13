#![no_main]

//! Fuzz target for EXIF parsing paths in lazy-image.
//! Exercises extract_exif_raw() and detect_exif_orientation() on arbitrary data.

use lazy_image::engine::{detect_exif_orientation, extract_exif_raw};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = extract_exif_raw(data);
    let _ = detect_exif_orientation(data);
});
