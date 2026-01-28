#![no_main]

use arbitrary::Arbitrary;
use image::ImageFormat;
use libfuzzer_sys::fuzz_target;
use std::io::{Cursor, Read};

#[derive(Arbitrary, Debug)]
struct StreamInput {
    data: Vec<u8>,
    chunk_size: usize,
}

fuzz_target!(|input: StreamInput| {
    let mut cursor = Cursor::new(input.data);
    let chunk = input.chunk_size.max(1).min(64 * 1024);
    let mut buf = vec![0u8; chunk];
    let mut aggregate = Vec::new();

    while let Ok(n) = cursor.read(&mut buf) {
        if n == 0 {
            break;
        }
        aggregate.extend_from_slice(&buf[..n]);
        // Try to guess format once we have some data
        if aggregate.len() > 8 {
            if let Ok(fmt) = image::guess_format(&aggregate) {
                if matches!(fmt, ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP | ImageFormat::Avif) {
                    let _ = image::load_from_memory_with_format(&aggregate, fmt);
                }
            }
        }
        if aggregate.len() > 2 * 1024 * 1024 {
            // avoid unbounded growth
            break;
        }
    }
});
