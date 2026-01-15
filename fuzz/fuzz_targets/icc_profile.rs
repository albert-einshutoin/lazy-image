#![no_main]

//! Fuzz target for ICC profile extraction and validation.
//! Tests ICC profile parsing from JPEG, PNG, WebP, and AVIF containers.

use lazy_image::engine::extract_icc_profile;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if data.len() < 8 {
        return;
    }

    // Test ICC extraction from raw data
    // The function handles JPEG, PNG, WebP, AVIF detection internally
    let _ = extract_icc_profile(data);

    // Also test with various magic byte prefixes to exercise different code paths

    // JPEG prefix (0xFF 0xD8)
    if data.len() > 2 {
        let mut jpeg_data = vec![0xFF, 0xD8];
        jpeg_data.extend_from_slice(data);
        let _ = extract_icc_profile(&jpeg_data);
    }

    // PNG prefix (0x89 PNG)
    if data.len() > 8 {
        let mut png_data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        png_data.extend_from_slice(data);
        let _ = extract_icc_profile(&png_data);
    }

    // WebP prefix (RIFF....WEBP)
    if data.len() > 12 {
        let mut webp_data = vec![
            0x52, 0x49, 0x46, 0x46, // RIFF
            0x00, 0x00, 0x00, 0x00, // size placeholder
            0x57, 0x45, 0x42, 0x50, // WEBP
        ];
        webp_data.extend_from_slice(data);
        let _ = extract_icc_profile(&webp_data);
    }

    // AVIF/HEIF prefix (ftyp)
    if data.len() > 12 {
        let mut avif_data = vec![
            0x00, 0x00, 0x00, 0x18, // box size
            0x66, 0x74, 0x79, 0x70, // ftyp
            0x61, 0x76, 0x69, 0x66, // avif
        ];
        avif_data.extend_from_slice(data);
        let _ = extract_icc_profile(&avif_data);
    }
});
