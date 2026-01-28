#![no_main]

use arbitrary::Arbitrary;
use lazy_image::engine::FirewallConfig;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct Input {
    max_pixels: Option<u64>,
    max_bytes: Option<u64>,
    timeout_ms: Option<u64>,
    width: u32,
    height: u32,
    size_bytes: usize,
}

fuzz_target!(|data: Input| {
    let mut fw = FirewallConfig::custom();
    fw.max_pixels = data.max_pixels;
    fw.max_bytes = data.max_bytes;
    fw.timeout_ms = data.timeout_ms;
    // enforce_pixels should never panic; invalid values must return errors
    let _ = fw.enforce_pixels(data.width, data.height);
    // enforce_source_len should respect max_bytes if set
    let _ = fw.enforce_source_len(data.size_bytes);
});
