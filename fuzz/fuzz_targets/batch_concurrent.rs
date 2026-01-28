#![no_main]

use arbitrary::Arbitrary;
use image::{ImageBuffer, Rgba};
use lazy_image::engine::encode_png;
use lazy_image::ops::OutputFormat;
use libfuzzer_sys::fuzz_target;
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;

#[derive(Arbitrary, Debug)]
struct Input {
    items: Vec<u8>,
    concurrency_hint: u32,
    format_byte: u8,
}

fn format_from_byte(b: u8) -> OutputFormat {
    match b % 4 {
        0 => OutputFormat::Jpeg { quality: 80, fast_mode: false },
        1 => OutputFormat::Png,
        2 => OutputFormat::WebP { quality: 75 },
        _ => OutputFormat::Avif { quality: 60 },
    }
}

fuzz_target!(|input: Input| {
    // Cap items to avoid huge allocations
    let items: Vec<u8> = input.items.into_iter().take(256).collect();
    if items.is_empty() {
        return;
    }

    let threads = (input.concurrency_hint as usize).clamp(1, 16);
    let pool = ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    let _format = format_from_byte(input.format_byte);

    // Parallel encode small synthetic images to exercise encoder + thread pool paths.
    let _sum: usize = pool.install(|| {
        items
            .par_iter()
            .map(|b| {
                let size = ((*b as u32) % 64).max(1);
                let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
                    ImageBuffer::from_pixel(size, size, Rgba([*b, b.wrapping_mul(3), 42, 255]));
                let dyn_img = image::DynamicImage::ImageRgba8(img);
                let _encoded = encode_png(&dyn_img, None).ok();
                size as usize
            })
            .sum()
    });
});
