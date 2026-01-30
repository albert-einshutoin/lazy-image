#![no_main]

use arbitrary::Arbitrary;
use image::ImageFormat;
use lazy_image::engine::{decode_with_image_crate, encode_png};
use libfuzzer_sys::fuzz_target;
use std::io::{Cursor, Read};

#[derive(Arbitrary, Debug)]
struct StreamInput {
    data: Vec<u8>,
    chunk_size: usize,
}

fuzz_target!(|input: StreamInput| {
    let mut cursor = Cursor::new(input.data);
    let chunk = input.chunk_size.max(1).min(32 * 1024);
    let mut buf = vec![0u8; chunk];
    let mut aggregate = Vec::new();

    while let Ok(n) = cursor.read(&mut buf) {
        if n == 0 {
            break;
        }
        aggregate.extend_from_slice(&buf[..n]);
        if aggregate.len() > 8 {
            if let Ok(fmt) = image::guess_format(&aggregate) {
                if matches!(fmt, ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP | ImageFormat::Avif) {
                    // Use lazy-image decode/encode path to exercise streaming-like incremental feeding.
                    if let Ok(img) = decode_with_image_crate(&aggregate) {
                        let _ = encode_png(&img, None);
                    }
                }
            }
        }
        if aggregate.len() > 2 * 1024 * 1024 {
            break; // avoid unbounded growth
        }
    }
});
