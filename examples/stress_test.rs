#![cfg(feature = "stress")]

use lazy_image::engine::run_stress_iteration;

const SAMPLE_JPEG: &[u8] = include_bytes!("../test/fixtures/test_input.jpg");
const SAMPLE_PNG: &[u8] = include_bytes!("../test/fixtures/test_100KB_1188x1188.png");
const SAMPLE_WEBP: &[u8] = include_bytes!("../test/fixtures/test_90KB_1471x1471.webp");

fn iterations_from_args() -> usize {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--iterations" || arg == "-n" {
            if let Some(value) = args.next() {
                if let Ok(parsed) = value.parse::<usize>() {
                    return parsed;
                }
            }
        } else if let Ok(parsed) = arg.parse::<usize>() {
            return parsed;
        }
    }
    200
}

fn main() {
    let iterations = iterations_from_args();

    for i in 0..iterations {
        run_or_fail("JPEG", i, SAMPLE_JPEG);
        run_or_fail("PNG", i, SAMPLE_PNG);
        run_or_fail("WebP", i, SAMPLE_WEBP);

        if i % 25 == 0 {
            eprintln!("stress iteration {i} completed");
        }
    }
}

fn run_or_fail(label: &str, iteration: usize, data: &[u8]) {
    if let Err(err) = run_stress_iteration(data) {
        panic!(
            "{} stress iteration {} failed: {}",
            label,
            iteration,
            err
        );
    }
}
