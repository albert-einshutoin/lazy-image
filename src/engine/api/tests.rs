// src/engine/api/tests.rs
//
// Test module body for `engine::api`, referenced via `#[path]` from `mod.rs`.
// Kept as a sibling file so the production modules stay focused.

use crate::engine::resize::fast_resize_owned;
#[allow(unused_imports)]
use image::GenericImageView;
use image::{DynamicImage, RgbImage};

fn create_test_image(width: u32, height: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    }))
}

#[test]
fn fast_resize_owned_returns_error_instead_of_dummy_image() {
    let img = create_test_image(1, 1);
    let err = fast_resize_owned(img, 0, 10).expect_err("expected resize failure");
    assert_eq!(err.source_dims, (1, 1));
    assert_eq!(err.target_dims, (0, 10));
    assert!(err.reason.contains("invalid dimensions"));
}
