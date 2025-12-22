use criterion::{black_box, criterion_group, criterion_main, Criterion};
// Note: Real benchmarking would import lazy_image::ImageEngine
// but setting up the NAPI environment for benchmarks is complex.
// This is a placeholder for the infrastructure.

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("fib 20", |b| b.iter(|| fibonacci(black_box(20))));
}

fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
