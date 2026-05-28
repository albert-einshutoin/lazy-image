// src/engine/io/test_helpers.rs
//
// Shared image fixtures for `io::icc` and `io::exif` unit tests.
//
// Kept under `#[cfg(test)]` at the `io` module level so siblings can reach
// the helpers via `super::super::test_helpers::*` without exposing them to
// production builds.

use image::{DynamicImage, RgbImage};
use std::io::Cursor;

/// Build a deterministic RGB test image.
pub(super) fn create_test_image(width: u32, height: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    }))
}

/// Produce a minimal valid JPEG via mozjpeg so format-aware parsers accept it.
pub(super) fn create_minimal_jpeg() -> Vec<u8> {
    let img = create_test_image(1, 1);
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let pixels = rgb.into_raw();

    let mut comp = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
    comp.set_size(w as usize, h as usize);
    comp.set_quality(80.0);
    comp.set_color_space(mozjpeg::ColorSpace::JCS_YCbCr);
    comp.set_chroma_sampling_pixel_sizes((2, 2), (2, 2));

    let mut output = Vec::new();
    {
        let mut writer = comp.start_compress(&mut output).unwrap();
        let stride = w as usize * 3;
        for row in pixels.chunks(stride) {
            writer.write_scanlines(row).unwrap();
        }
        writer.finish().unwrap();
    }
    output
}

/// Produce a minimal valid PNG using the `image` crate.
pub(super) fn create_minimal_png() -> Vec<u8> {
    let img = create_test_image(1, 1);
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    buf
}

/// Produce a minimal valid WebP using libwebp.
pub(super) fn create_minimal_webp() -> Vec<u8> {
    let img = create_test_image(10, 10);
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let encoder = webp::Encoder::from_rgb(&rgb, w, h);
    let config = webp::WebPConfig::new().unwrap();
    let mem = encoder.encode_advanced(&config).unwrap();
    mem.to_vec()
}
