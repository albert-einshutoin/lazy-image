#![no_main]

use arbitrary::Arbitrary;
use image::{ImageBuffer, Rgba};
use lazy_image::engine::{encode_png, encode_webp};
use lazy_image::ProcessingMetrics;
use libfuzzer_sys::fuzz_target;
use std::time::Instant;

#[derive(Arbitrary, Debug)]
struct MetricsInput {
    size: u8,
    quality: u8,
}

fuzz_target!(|m: MetricsInput| {
    let dim = (m.size as u32).max(1);
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(dim, dim, Rgba([m.size, m.quality, 42, 255]));
    let dyn_img = image::DynamicImage::ImageRgba8(img);

    let mut metrics = ProcessingMetrics::default();
    metrics.bytes_in = (dim * dim * 4).min(u32::MAX);
    let start = Instant::now();

    // Encode twice to exercise metrics accumulation paths
    let _ = encode_png(&dyn_img, None);
    metrics.decode_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mid = Instant::now();
    let _ = encode_webp(&dyn_img, (m.quality % 100).max(1), None);
    metrics.encode_ms = mid.elapsed().as_secs_f64() * 1000.0;
    metrics.total_ms = start.elapsed().as_secs_f64() * 1000.0;

    metrics.bytes_out = (dim * dim).min(u32::MAX);
    metrics.compression_ratio = if metrics.bytes_in == 0 {
        0.0
    } else {
        metrics.bytes_out as f64 / metrics.bytes_in as f64
    };
    metrics.processing_time = metrics.total_ms / 1000.0;
});
