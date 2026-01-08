// benches/benchmark.rs
//
// Performance benchmarks for lazy-image
// Run with: cargo bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};

// =============================================================================
// THREAD POOL BENCHMARKS
// =============================================================================

/// Benchmark: Global thread pool vs creating new pools
///
/// This measures the overhead of creating thread pools per-request
/// versus using a global pool (which lazy-image does by default).
fn bench_thread_pool_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("thread_pool");

    // Simulate workload sizes
    let workload_sizes = [10, 100, 1000];

    for size in workload_sizes {
        // Benchmark: Using rayon's global pool (default behavior)
        group.bench_with_input(BenchmarkId::new("global_pool", size), &size, |b, &size| {
            b.iter(|| {
                let counter = AtomicUsize::new(0);
                (0..size).into_par_iter().for_each(|_| {
                    // Simulate light work
                    counter.fetch_add(1, Ordering::Relaxed);
                    black_box(fibonacci(10));
                });
                counter.load(Ordering::Relaxed)
            });
        });

        // Benchmark: Creating a new pool per operation (what we want to avoid)
        group.bench_with_input(
            BenchmarkId::new("new_pool_per_call", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let pool = rayon::ThreadPoolBuilder::new()
                        .num_threads(4)
                        .build()
                        .unwrap();

                    let counter = AtomicUsize::new(0);
                    pool.install(|| {
                        (0..size).into_par_iter().for_each(|_| {
                            counter.fetch_add(1, Ordering::Relaxed);
                            black_box(fibonacci(10));
                        });
                    });
                    counter.load(Ordering::Relaxed)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Parallel vs sequential processing
///
/// Shows the benefit of parallel processing for batch operations.
fn bench_parallel_vs_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_processing");

    let workload_sizes = [10, 100, 500];

    for size in workload_sizes {
        // Sequential processing
        group.bench_with_input(BenchmarkId::new("sequential", size), &size, |b, &size| {
            b.iter(|| {
                let mut results = Vec::with_capacity(size);
                for i in 0..size {
                    results.push(black_box(fibonacci(15)));
                    black_box(i);
                }
                results
            });
        });

        // Parallel processing with rayon
        group.bench_with_input(BenchmarkId::new("parallel", size), &size, |b, &size| {
            b.iter(|| {
                let results: Vec<u64> = (0..size)
                    .into_par_iter()
                    .map(|i| {
                        black_box(i);
                        black_box(fibonacci(15))
                    })
                    .collect();
                results
            });
        });
    }

    group.finish();
}

/// Benchmark: Thread pool with different concurrency levels
fn bench_concurrency_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrency_levels");

    let workload = 100;
    let concurrency_levels = [1, 2, 4, 8];

    for threads in concurrency_levels {
        group.bench_with_input(
            BenchmarkId::new("threads", threads),
            &threads,
            |b, &threads| {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();

                b.iter(|| {
                    pool.install(|| {
                        let results: Vec<u64> = (0..workload)
                            .into_par_iter()
                            .map(|_| black_box(fibonacci(15)))
                            .collect();
                        results
                    })
                });
            },
        );
    }

    group.finish();
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Simple CPU-bound work for benchmarking
fn fibonacci(n: u64) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

// =============================================================================
// BENCHMARK GROUPS
// =============================================================================

criterion_group!(
    benches,
    bench_thread_pool_overhead,
    bench_parallel_vs_sequential,
    bench_concurrency_levels,
);

criterion_main!(benches);
