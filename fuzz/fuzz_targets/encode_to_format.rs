#![no_main]

//! Fuzz target for image encoding to various formats.
//! Tests JPEG (mozjpeg), PNG, WebP, and AVIF encoding paths for crashes and memory issues.

use arbitrary::{Arbitrary, Unstructured};
use image::{DynamicImage, RgbaImage};
use lazy_image::engine::{encode_avif, encode_jpeg, encode_png, encode_webp};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct EncodeSeed {
    format: u8,
    quality: u8,
    width: u8,
    height: u8,
}

fn build_image(data: &[u8], width: u8, height: u8) -> DynamicImage {
    // Limit dimensions to avoid OOM (max 128x128 = 64KB RGBA)
    let w = (width as u32 % 128).max(1);
    let h = (height as u32 % 128).max(1);
    let pixel_count = (w * h * 4) as usize;

    let mut buffer = vec![0u8; pixel_count];
    for (i, byte) in buffer.iter_mut().enumerate() {
        *byte = data.get(i % data.len().max(1)).copied().unwrap_or(128);
    }

    let rgba = RgbaImage::from_raw(w, h, buffer)
        .unwrap_or_else(|| RgbaImage::from_raw(1, 1, vec![0, 0, 0, 255]).unwrap());
    DynamicImage::ImageRgba8(rgba)
}

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 {
        return;
    }

    let mut unstructured = Unstructured::new(data);
    let seed: EncodeSeed = match EncodeSeed::arbitrary(&mut unstructured) {
        Ok(s) => s,
        Err(_) => return,
    };

    let img = build_image(data, seed.width, seed.height);
    let quality = seed.quality.clamp(1, 100);

    // Test encoding to different formats
    // We only care about panics/crashes, not encoding errors
    match seed.format % 4 {
        0 => {
            // JPEG encoding via mozjpeg
            let _ = encode_jpeg(&img, quality, None);
        }
        1 => {
            // PNG encoding
            let _ = encode_png(&img, None);
        }
        2 => {
            // WebP encoding via libwebp
            let _ = encode_webp(&img, quality, None);
        }
        _ => {
            // AVIF encoding via libavif (AOMedia reference encoder)
            let _ = encode_avif(&img, quality, None);
        }
    }
});
