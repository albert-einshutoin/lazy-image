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
    // Clamp to avoid pathological allocations / overflows inside error messages.
    let clamp_u64 = |v: Option<u64>, cap: u64| v.map(|x| x.min(cap));
    let max_px = clamp_u64(data.max_pixels, 100_000_000); // 100 MP
    let max_bytes = clamp_u64(data.max_bytes, 512 * 1024 * 1024); // 512 MB
    let size_bytes = data.size_bytes.min(512 * 1024 * 1024); // 512 MB
    let width = data.width.min(50_000);
    let height = data.height.min(50_000);

    let mut fw = FirewallConfig::custom();
    fw.max_pixels = max_px;
    fw.max_bytes = max_bytes;
    fw.timeout_ms = data.timeout_ms;
    let _ = fw.enforce_pixels(width, height);
    let _ = fw.enforce_source_len(size_bytes);
});
