// Shared test fixtures for `engine` unit tests.
//
// This module is compiled only under `cfg(test)`. Shared fixtures here reduce
// copy/paste drift across module-local test modules.

use image::{DynamicImage, RgbImage, RgbaImage};

/// Deterministic RGB test image (gradient pattern).
pub(crate) fn create_test_image(width: u32, height: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    }))
}

/// Deterministic RGBA test image (gradient pattern, fully opaque).
pub(crate) fn create_test_image_rgba(width: u32, height: u32) -> DynamicImage {
    DynamicImage::ImageRgba8(RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
    }))
}

/// Minimal valid ICC profile used by several encoder tests.
pub(crate) fn minimal_icc_profile() -> Vec<u8> {
    let mut icc = vec![0u8; 132];
    icc[0..4].copy_from_slice(&132u32.to_be_bytes());
    icc[4..8].copy_from_slice(b"ADBE");
    icc[8] = 2;
    icc[12..16].copy_from_slice(b"mntr");
    icc[16..20].copy_from_slice(b"RGB ");
    icc[20..24].copy_from_slice(b"XYZ ");
    icc[36..40].copy_from_slice(b"acsp");
    icc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_icc_profile_matches_encoder_fixture() {
        let mut expected = vec![0u8; 132];
        expected[0..4].copy_from_slice(&132u32.to_be_bytes());
        expected[4..8].copy_from_slice(b"ADBE");
        expected[8] = 2;
        expected[12..16].copy_from_slice(b"mntr");
        expected[16..20].copy_from_slice(b"RGB ");
        expected[20..24].copy_from_slice(b"XYZ ");
        expected[36..40].copy_from_slice(b"acsp");

        assert_eq!(minimal_icc_profile(), expected);
    }
}
