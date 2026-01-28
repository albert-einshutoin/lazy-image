#![no_main]

use arbitrary::Arbitrary;
use lazy_image::ops::OutputFormat;
use libfuzzer_sys::fuzz_target;
use rayon::prelude::*;

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
    // Exercise rayon scheduling with a simple map/reduce workload.
    let _format = format_from_byte(input.format_byte);
    let sum: usize = items.par_iter().map(|b| (*b as usize) ^ 0xA5).sum();

    // Avoid unused warnings
    if sum == usize::MAX {
        panic!("unreachable");
    }
});
