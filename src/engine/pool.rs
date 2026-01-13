// src/engine/pool.rs
//
// Global thread pool management for batch processing.
//
// **Architecture Decision**: We use a single global thread pool for all batch
// operations instead of creating a new pool per request. This provides:
//
// 1. **Zero allocation overhead**: No pool creation cost per batch
// 2. **Better resource utilization**: Threads are reused across operations
// 3. **Predictable performance**: Consistent thread count based on CPU cores
//
// **Thread Count Calculation**:
// - Uses std::thread::available_parallelism() to respect cgroup/CPU quota
// - Reserves UV_THREADPOOL_SIZE threads for libuv (defaults to 4) to avoid oversubscription
// - Considers memory limits for smart concurrency (see memory.rs)
// - Fallback is MIN_RAYON_THREADS when detection fails
//
// **IMPORTANT**:
// - Pool is initialized lazily on first use
// - Changes after initialization have NO effect
//
// **Benchmark Results** (see benches/benchmark.rs):
// - Global pool: ~0.5ms overhead for 100 items
// - New pool per call: ~5-10ms overhead (10-20x slower)

#[cfg(feature = "napi")]
use crate::engine::memory;
#[cfg(feature = "napi")]
use rayon::ThreadPool;
#[cfg(feature = "napi")]
use std::sync::OnceLock;

/// Default libuv thread pool size (Node.js default)
#[cfg(feature = "napi")]
const DEFAULT_LIBUV_THREADPOOL_SIZE: usize = 4;

/// Maximum allowed concurrency value for processBatch()
#[cfg(feature = "napi")]
pub const MAX_CONCURRENCY: usize = 1024;

/// Minimum number of rayon threads to ensure at least some parallelism
#[cfg(feature = "napi")]
const MIN_RAYON_THREADS: usize = 1;

#[cfg(feature = "napi")]
pub(crate) static GLOBAL_THREAD_POOL: OnceLock<ThreadPool> = OnceLock::new();

#[cfg(feature = "napi")]
pub fn get_pool() -> &'static ThreadPool {
    GLOBAL_THREAD_POOL.get_or_init(|| {
        let detected_parallelism = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(MIN_RAYON_THREADS);

        let uv_reserve = reserved_libuv_threads();
        let num_threads = detected_parallelism
            .saturating_sub(uv_reserve)
            .max(MIN_RAYON_THREADS);

        rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .unwrap_or_else(|e| {
                // Fallback: create a minimal thread pool if the preferred configuration fails
                rayon::ThreadPoolBuilder::new()
                    .num_threads(MIN_RAYON_THREADS)
                    .build()
                    .expect(&format!(
                        "Failed to create fallback thread pool with {} threads: {}",
                        MIN_RAYON_THREADS, e
                    ))
            })
    })
}

/// Calculates optimal concurrency based on CPU and memory constraints
///
/// This function combines CPU-based parallelism detection with memory-aware
/// concurrency limits to prevent OOM kills in constrained containers.
///
/// # Returns
/// Optimal concurrency value (number of concurrent operations)
#[cfg(feature = "napi")]
pub fn calculate_optimal_concurrency() -> usize {
    // 1. Detect CPU-based parallelism
    let cpu_based = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(MIN_RAYON_THREADS);

    // Reserve threads for libuv
    let uv_reserve = reserved_libuv_threads();
    let cpu_concurrency = cpu_based
        .saturating_sub(uv_reserve)
        .max(MIN_RAYON_THREADS);

    // 2. Detect memory limits and calculate memory-based concurrency
    let available_memory = memory::detect_available_memory();
    let memory_based = memory::calculate_memory_based_concurrency(available_memory, cpu_concurrency);

    // 3. Use the minimum of CPU and memory constraints
    // This ensures we don't exceed either CPU or memory limits
    memory_based
}

#[cfg(feature = "napi")]
fn reserved_libuv_threads() -> usize {
    std::env::var("UV_THREADPOOL_SIZE")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(DEFAULT_LIBUV_THREADPOOL_SIZE)
}
