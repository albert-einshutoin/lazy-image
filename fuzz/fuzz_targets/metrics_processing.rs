#![no_main]

use arbitrary::Arbitrary;
use lazy_image::ProcessingMetrics;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct MetricsInput {
    decode: f64,
    ops: f64,
    encode: f64,
    total: f64,
    bytes_in: u32,
    bytes_out: u32,
    cpu_time: f64,
}

fuzz_target!(|m: MetricsInput| {
    let mut metrics = ProcessingMetrics::default();
    metrics.decode_ms = m.decode.abs();
    metrics.ops_ms = m.ops.abs();
    metrics.encode_ms = m.encode.abs();
    metrics.total_ms = m.total.abs();
    metrics.bytes_in = m.bytes_in;
    metrics.bytes_out = m.bytes_out;
    metrics.cpu_time = m.cpu_time.abs();
    metrics.processing_time = metrics.total_ms / 1000.0;
    // Derived fields should not panic
    metrics.compression_ratio = if metrics.bytes_in == 0 {
        0.0
    } else {
        metrics.bytes_out as f64 / metrics.bytes_in as f64
    };
    // Copy to legacy aliases
    metrics.decode_time = metrics.decode_ms;
    metrics.process_time = metrics.ops_ms;
    metrics.encode_time = metrics.encode_ms;
    metrics.memory_peak = metrics.peak_rss;
    metrics.input_size = metrics.bytes_in;
    metrics.output_size = metrics.bytes_out;
});
