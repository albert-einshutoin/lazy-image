#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use image::{self, DynamicImage, RgbaImage};
use lazy_image::engine::EncodeTask;
use lazy_image::ops::{ColorSpace, Operation};
use libfuzzer_sys::fuzz_target;
use std::borrow::Cow;

#[derive(Arbitrary, Debug)]
struct OperationSeed {
    kind: u8,
    a: i32,
    b: i32,
    c: i32,
    d: i32,
}

fn build_image(data: &[u8]) -> DynamicImage {
    if let Ok(img) = image::load_from_memory(data) {
        return img;
    }

    let width = data.get(0).copied().unwrap_or(0) as u32 % 64 + 1;
    let height = data.get(1).copied().unwrap_or(0) as u32 % 64 + 1;
    let mut buffer = vec![0u8; (width * height * 4) as usize];
    for (i, byte) in buffer.iter_mut().enumerate() {
        *byte = data.get(i % data.len()).copied().unwrap_or(0);
    }

    let rgba = RgbaImage::from_raw(width, height, buffer)
        .unwrap_or_else(|| RgbaImage::from_raw(1, 1, vec![0, 0, 0, 255]).unwrap());
    DynamicImage::ImageRgba8(rgba)
}

fn seeds_to_ops(seeds: Vec<OperationSeed>) -> Vec<Operation> {
    seeds
        .into_iter()
        .take(16)
        .map(|seed| match seed.kind % 8 {
            0 => Operation::Resize {
                width: Some(seed.a.clamp(1, 4096) as u32),
                height: Some(seed.b.clamp(1, 4096) as u32),
            },
            1 => Operation::Crop {
                x: seed.a.max(0) as u32,
                y: seed.b.max(0) as u32,
                width: seed.c.max(1) as u32,
                height: seed.d.max(1) as u32,
            },
            2 => Operation::Rotate { degrees: seed.a },
            3 => Operation::FlipH,
            4 => Operation::FlipV,
            5 => Operation::Brightness {
                value: seed.a.clamp(-200, 200),
            },
            6 => Operation::Contrast {
                value: seed.b.clamp(-200, 200),
            },
            _ => Operation::ColorSpace {
                target: ColorSpace::Srgb,
            },
        })
        .collect()
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    let mut unstructured = Unstructured::new(data);
    let seeds: Vec<OperationSeed> = match Vec::arbitrary(&mut unstructured) {
        Ok(v) => v,
        Err(_) => return,
    };

    let ops = seeds_to_ops(seeds);
    let img = build_image(data);
    // Fuzzing: apply_ops may return errors for invalid operations; we're
    // interested only in panics or memory issues.
    let _ = EncodeTask::apply_ops(Cow::Owned(img), &ops);
});
